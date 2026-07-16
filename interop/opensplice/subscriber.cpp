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
    return 90;
}

static int interop_timeout_ms()
{
    const char *env = std::getenv("AIDDS_INTEROP_TIMEOUT_MS");
    if (env != nullptr && env[0] != '\0')
        return std::atoi(env);
    return 8000;
}

int main(int argc, char **argv)
{
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

    dds::domain::DomainParticipant participant(domain_id);
    dds::topic::Topic<AiDdsInterop::Message> topic(
        participant, "AiDdsInteropMessage", "AiDdsInterop::Message");
    dds::sub::Subscriber subscriber(participant);

    dds::sub::qos::DataReaderQos drqos;
    drqos << dds::core::policy::Reliability::Reliable();
    dds::sub::DataReader<AiDdsInterop::Message> reader(subscriber, topic, drqos);

    while (elapsed < timeout_ms)
    {
        dds::sub::LoanedSamples<AiDdsInterop::Message> samples = reader.take();
        for (const auto &sample : samples)
        {
            if (!sample.info().valid())
                continue;

            const AiDdsInterop::Message &msg = sample.data();
            if (have_expect_id && msg.id() != expect_id)
            {
                std::printf(
                    "INTEROP_RECEIVE id=%u payload=%s (ignored, expected %lu)\n",
                    msg.id(), msg.payload().c_str(), expect_id);
                continue;
            }

            std::printf(
                "INTEROP_RECEIVE id=%u payload=%s domain=%d\n",
                msg.id(), msg.payload().c_str(), domain_id);
            std::fflush(stdout);
            return EXIT_SUCCESS;
        }

        std::this_thread::sleep_for(std::chrono::milliseconds(20));
        elapsed += 20;
    }

    std::fprintf(stderr, "INTEROP_TIMEOUT after %d ms\n", timeout_ms);
    return EXIT_FAILURE;
}
