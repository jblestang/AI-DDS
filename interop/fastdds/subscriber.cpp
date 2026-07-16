#include <chrono>
#include <cstdlib>
#include <cstdio>
#include <cstring>
#include <thread>

#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/subscriber/DataReader.hpp>
#include <fastdds/dds/subscriber/SampleInfo.hpp>
#include <fastdds/dds/subscriber/Subscriber.hpp>
#include <fastdds/dds/subscriber/qos/DataReaderQos.hpp>
#include <fastdds/dds/topic/Topic.hpp>
#include <fastdds/dds/topic/TypeSupport.hpp>
#include <fastdds/dds/subscriber/DataReaderListener.hpp>
#include <fastrtps/types/TypesBase.h>

#include "InteropMessagePubSubTypes.h"

using namespace eprosima::fastdds::dds;
using eprosima::fastrtps::types::ReturnCode_t;

class InteropReaderListener : public DataReaderListener
{
public:
    void on_subscription_matched(
            DataReader *,
            const SubscriptionMatchedStatus &info) override
    {
        std::printf(
            "INTEROP_SUBSCRIPTION_MATCHED current=%u total=%u change=%d\n",
            info.current_count, info.total_count, info.current_count_change);
        std::fflush(stdout);
    }
};

static InteropReaderListener g_reader_listener;

static int interop_domain_id()
{
    const char *env = std::getenv("AIDDS_INTEROP_DOMAIN");
    if (env != nullptr && env[0] != '\0')
        return std::atoi(env);
    return 83;
}

static int interop_timeout_ms()
{
    const char *env = std::getenv("AIDDS_INTEROP_TIMEOUT_MS");
    if (env != nullptr && env[0] != '\0')
        return std::atoi(env);
    return 8000;
}

static void configure_fastdds_udp_profile()
{
    if (std::getenv("AIDDS_FASTDDS_NO_PROFILE") != nullptr)
        return;
    if (std::getenv("FASTRTPS_DEFAULT_PROFILES_FILE") == nullptr)
    {
        setenv("FASTRTPS_DEFAULT_PROFILES_FILE", AIDDS_FASTDDS_INTEROP_PROFILE, 0);
    }
}

int main(int argc, char **argv)
{
    configure_fastdds_udp_profile();
    int domain_id = interop_domain_id();
    int timeout_ms = interop_timeout_ms();
    int elapsed = 0;
    unsigned long expect_id = 0;
    bool have_expect_id = false;

    if (argc > 1)
    {
        expect_id = std::strtoul(argv[1], nullptr, 10);
        have_expect_id = true;
    }

    DomainParticipant *participant = DomainParticipantFactory::get_instance()->create_participant(
        domain_id, PARTICIPANT_QOS_DEFAULT);
    if (participant == nullptr)
    {
        std::fprintf(stderr, "create_participant failed\n");
        return EXIT_FAILURE;
    }

    TypeSupport type(new AiDdsInterop::MessagePubSubType());
    if (type.register_type(participant) != ReturnCode_t::RETCODE_OK)
    {
        std::fprintf(stderr, "register_type failed\n");
        return EXIT_FAILURE;
    }

    Topic *topic = participant->create_topic(
        "AiDdsInteropMessage", "AiDdsInterop::Message", TOPIC_QOS_DEFAULT);
    if (topic == nullptr)
    {
        std::fprintf(stderr, "create_topic failed\n");
        return EXIT_FAILURE;
    }

    Subscriber *subscriber = participant->create_subscriber(SUBSCRIBER_QOS_DEFAULT);
    if (subscriber == nullptr)
    {
        std::fprintf(stderr, "create_subscriber failed\n");
        return EXIT_FAILURE;
    }

    DataReaderQos rqos = DATAREADER_QOS_DEFAULT;
    rqos.reliability().kind = RELIABLE_RELIABILITY_QOS;
    DataReader *reader = subscriber->create_datareader(topic, rqos, &g_reader_listener);
    if (reader == nullptr)
    {
        std::fprintf(stderr, "create_datareader failed\n");
        return EXIT_FAILURE;
    }

    AiDdsInterop::Message msg;
    SampleInfo info;

    while (elapsed < timeout_ms)
    {
        if (reader->take_next_sample(&msg, &info) == ReturnCode_t::RETCODE_OK && info.valid_data)
        {
            if (have_expect_id && msg.id() != expect_id)
            {
                std::printf(
                    "INTEROP_RECEIVE id=%u payload=%s (ignored, expected %lu)\n",
                    msg.id(), msg.payload().c_str(), expect_id);
            }
            else
            {
                std::printf(
                    "INTEROP_RECEIVE id=%u payload=%s domain=%d\n",
                    msg.id(), msg.payload().c_str(), domain_id);
                std::fflush(stdout);
                participant->delete_contained_entities();
                DomainParticipantFactory::get_instance()->delete_participant(participant);
                return EXIT_SUCCESS;
            }
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(20));
        elapsed += 20;
    }

    participant->delete_contained_entities();
    DomainParticipantFactory::get_instance()->delete_participant(participant);
    std::fprintf(stderr, "INTEROP_TIMEOUT after %d ms\n", timeout_ms);
    return EXIT_FAILURE;
}
