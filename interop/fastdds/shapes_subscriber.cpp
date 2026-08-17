#include <chrono>
#include <cstdlib>
#include <cstdio>
#include <cstring>
#include <string>
#include <thread>

#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/subscriber/DataReader.hpp>
#include <fastdds/dds/subscriber/SampleInfo.hpp>
#include <fastdds/dds/subscriber/Subscriber.hpp>
#include <fastdds/dds/subscriber/qos/DataReaderQos.hpp>
#include <fastdds/dds/topic/Topic.hpp>
#include <fastdds/dds/topic/TypeSupport.hpp>
#include <fastrtps/types/TypesBase.h>

#include "ShapeTypePubSubTypes.h"

using namespace eprosima::fastdds::dds;
using eprosima::fastrtps::types::ReturnCode_t;

static int shapes_domain_id()
{
    const char *env = std::getenv("AIDDS_SHAPES_DOMAIN");
    if (env != nullptr && env[0] != '\0')
        return std::atoi(env);
    return 120;
}

static int shapes_timeout_ms()
{
    const char *env = std::getenv("AIDDS_SHAPES_TIMEOUT_MS");
    if (env != nullptr && env[0] != '\0')
        return std::atoi(env);
    return 12000;
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

    const char *topic_name = (argc > 1 && argv[1][0] != '\0') ? argv[1] : "Square";
    const char *expect_color = (argc > 2) ? argv[2] : nullptr;
    int domain_id = shapes_domain_id();
    int timeout_ms = shapes_timeout_ms();

    DomainParticipant *participant = DomainParticipantFactory::get_instance()->create_participant(
        domain_id, PARTICIPANT_QOS_DEFAULT);
    if (participant == nullptr)
    {
        std::fprintf(stderr, "create_participant failed\n");
        return EXIT_FAILURE;
    }

    TypeSupport type(new org::omg::dds::demo::ShapeTypePubSubType());
    if (type.register_type(participant) != ReturnCode_t::RETCODE_OK)
    {
        std::fprintf(stderr, "register_type failed\n");
        return EXIT_FAILURE;
    }

    Topic *topic = participant->create_topic(
        topic_name, "org::omg::dds::demo::ShapeType", TOPIC_QOS_DEFAULT);
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
    DataReader *reader = subscriber->create_datareader(topic, rqos);
    if (reader == nullptr)
    {
        std::fprintf(stderr, "create_datareader failed\n");
        return EXIT_FAILURE;
    }

    org::omg::dds::demo::ShapeType msg;
    SampleInfo info;
    const auto start = std::chrono::steady_clock::now();

    while (true)
    {
        const auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(
            std::chrono::steady_clock::now() - start);
        if (elapsed.count() >= timeout_ms)
            break;

        if (reader->take_next_sample(&msg, &info) == ReturnCode_t::RETCODE_OK && info.valid_data)
        {
            const std::string &color = msg.color();
            if (expect_color != nullptr && color != expect_color)
            {
                std::printf(
                    "SHAPES_RECEIVE topic=%s color=%s x=%d y=%d shapesize=%d (ignored, expected %s)\n",
                    topic_name, color.c_str(), static_cast<int>(msg.x()), static_cast<int>(msg.y()),
                    static_cast<int>(msg.shapesize()), expect_color);
            }
            else
            {
                std::printf(
                    "SHAPES_RECEIVE topic=%s color=%s x=%d y=%d shapesize=%d domain=%d\n",
                    topic_name, color.c_str(), static_cast<int>(msg.x()), static_cast<int>(msg.y()),
                    static_cast<int>(msg.shapesize()), domain_id);
                std::fflush(stdout);
                participant->delete_contained_entities();
                DomainParticipantFactory::get_instance()->delete_participant(participant);
                return EXIT_SUCCESS;
            }
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(20));
    }

    participant->delete_contained_entities();
    DomainParticipantFactory::get_instance()->delete_participant(participant);
    std::fprintf(stderr, "SHAPES_TIMEOUT after %d ms\n", timeout_ms);
    return EXIT_FAILURE;
}
