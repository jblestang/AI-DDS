#include <chrono>
#include <cstdlib>
#include <cstdio>
#include <cstring>
#include <string>
#include <thread>

#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/publisher/DataWriter.hpp>
#include <fastdds/dds/publisher/Publisher.hpp>
#include <fastdds/dds/publisher/qos/DataWriterQos.hpp>
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

static int shapes_wait_match()
{
    const char *env = std::getenv("AIDDS_SHAPES_WAIT_MATCH");
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

    const char *topic_name = (argc > 1 && argv[1][0] != '\0') ? argv[1] : "Square";
    const char *color = (argc > 2) ? argv[2] : "RED";
    int32_t x = (argc > 3) ? std::strtol(argv[3], nullptr, 10) : 10;
    int32_t y = (argc > 4) ? std::strtol(argv[4], nullptr, 10) : 20;
    int32_t shapesize = (argc > 5) ? std::strtol(argv[5], nullptr, 10) : 30;
    int domain_id = shapes_domain_id();

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

    if (shapes_wait_match())
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

    org::omg::dds::demo::ShapeType msg;
    msg.color(std::string(color));
    msg.x(x);
    msg.y(y);
    msg.shapesize(shapesize);

    std::printf(
        "SHAPES_PUBLISH topic=%s color=%s x=%d y=%d shapesize=%d domain=%d\n",
        topic_name, color, static_cast<int>(x), static_cast<int>(y), static_cast<int>(shapesize), domain_id);
    std::fflush(stdout);

    const auto write_rc = writer->write(&msg);
    if (write_rc != ReturnCode_t::RETCODE_OK)
    {
        std::fprintf(stderr, "write failed\n");
        return EXIT_FAILURE;
    }

    std::this_thread::sleep_for(std::chrono::milliseconds(2000));

    participant->delete_contained_entities();
    DomainParticipantFactory::get_instance()->delete_participant(participant);
    return EXIT_SUCCESS;
}
