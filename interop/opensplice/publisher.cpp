#include <chrono>
#include <cstdlib>
#include <cstdio>
#include <cstring>
#include <thread>

#include "dds/dds.hpp"
#include "AiDdsInterop/Message.hpp"

static int interop_domain_id()
{
    const char *env = std::getenv("AIDDS_INTEROP_DOMAIN");
    if (env != nullptr && env[0] != '\0')
        return std::atoi(env);
    return 93;
}

static int interop_wait_match()
{
    const char *env = std::getenv("AIDDS_INTEROP_WAIT_MATCH");
    if (env != nullptr && env[0] != '\0')
        return std::atoi(env);
    return 1;
}

int main(int argc, char **argv)
{
    unsigned long sample_id = 4242;
    const char *payload = "opensplice-to-aidds";
    int domain_id = interop_domain_id();

    if (argc > 1)
        sample_id = std::strtoul(argv[1], nullptr, 10);
    if (argc > 2)
        payload = argv[2];

    dds::domain::DomainParticipant participant(domain_id);
    dds::topic::Topic<AiDdsInterop::Message> topic(
        participant, "AiDdsInteropMessage", "AiDdsInterop::Message");
    dds::pub::Publisher publisher(participant);

    dds::pub::qos::DataWriterQos dwqos;
    dwqos << dds::core::policy::Reliability::Reliable();
    dds::pub::DataWriter<AiDdsInterop::Message> writer(publisher, topic, dwqos);

    if (interop_wait_match())
    {
        while (writer.publication_matched_status().current_count() == 0)
            std::this_thread::sleep_for(std::chrono::milliseconds(20));
    }

    AiDdsInterop::Message msg;
    msg.id(static_cast<uint32_t>(sample_id));
    msg.payload(payload);

    std::printf(
        "INTEROP_PUBLISH id=%lu payload=%s domain=%d\n", sample_id, payload, domain_id);
    std::fflush(stdout);

    writer.write(msg);
    std::this_thread::sleep_for(std::chrono::milliseconds(500));
    return EXIT_SUCCESS;
}
