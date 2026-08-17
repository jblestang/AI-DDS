#include <chrono>
#include <cstdlib>
#include <cstdio>
#include <cstring>
#include <thread>

#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/publisher/DataWriter.hpp>
#include <fastdds/dds/publisher/Publisher.hpp>
#include <fastdds/dds/publisher/qos/DataWriterQos.hpp>
#include <fastdds/dds/topic/Topic.hpp>
#include <fastdds/dds/topic/TypeSupport.hpp>
#include <fastrtps/types/TypesBase.h>

#include "InteropMessagePubSubTypes.h"

using namespace eprosima::fastdds::dds;
using eprosima::fastrtps::types::ReturnCode_t;

static int interop_domain_id()
{
    const char *env = std::getenv("AIDDS_INTEROP_DOMAIN");
    if (env != nullptr && env[0] != '\0')
        return std::atoi(env);
    return 83;
}

static int interop_wait_match()
{
    const char *env = std::getenv("AIDDS_INTEROP_WAIT_MATCH");
    if (env != nullptr && env[0] != '\0')
        return std::atoi(env);
    return 1;
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
    unsigned long sample_id = 4242;
    const char *payload = "fastdds-to-aidds";
    int domain_id = interop_domain_id();

    if (argc > 1)
        sample_id = std::strtoul(argv[1], nullptr, 10);
    if (argc > 2)
        payload = argv[2];

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

    Publisher *publisher = participant->create_publisher(PUBLISHER_QOS_DEFAULT);
    if (publisher == nullptr)
    {
        std::fprintf(stderr, "create_publisher failed\n");
        return EXIT_FAILURE;
    }

    DataWriterQos wqos = DATAWRITER_QOS_DEFAULT;
    wqos.reliability().kind = RELIABLE_RELIABILITY_QOS;
    DataWriter *writer = publisher->create_datawriter(topic, wqos);
    if (writer == nullptr)
    {
        std::fprintf(stderr, "create_datawriter failed\n");
        return EXIT_FAILURE;
    }

    if (interop_wait_match())
    {
        PublicationMatchedStatus status{};
        while (status.current_count == 0)
        {
            if (writer->get_publication_matched_status(status) != ReturnCode_t::RETCODE_OK)
            {
                std::fprintf(stderr, "get_publication_matched_status failed\n");
                return EXIT_FAILURE;
            }
            std::this_thread::sleep_for(std::chrono::milliseconds(20));
        }
    }

    AiDdsInterop::Message msg;
    msg.id(static_cast<uint32_t>(sample_id));
    msg.payload(std::string(payload));

    std::printf(
        "INTEROP_PUBLISH id=%lu payload=%s domain=%d\n", sample_id, payload, domain_id);
    std::fflush(stdout);

    const auto write_rc = writer->write(&msg);
    if (write_rc != ReturnCode_t::RETCODE_OK)
    {
        std::fprintf(stderr, "write failed (ReturnCode=%d)\n", static_cast<int>(write_rc));
        return EXIT_FAILURE;
    }

    std::this_thread::sleep_for(std::chrono::milliseconds(2000));

    participant->delete_contained_entities();
    DomainParticipantFactory::get_instance()->delete_participant(participant);
    return EXIT_SUCCESS;
}
