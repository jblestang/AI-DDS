/*
 * OpenDDS interop subscriber for AI-DDS harness (InteropMessage.idl).
 */

#include "DataReaderListener.h"
#include "InteropMessageTypeSupportImpl.h"

#include <ace/Log_Msg.h>
#include <ace/OS_NS_stdlib.h>
#include <ace/OS_NS_unistd.h>

#include <dds/DdsDcpsInfrastructureC.h>
#include <dds/DdsDcpsSubscriptionC.h>
#include <dds/DCPS/Marked_Default_Qos.h>
#include <dds/DCPS/Service_Participant.h>
#include <dds/DCPS/WaitSet.h>
#include <dds/DCPS/StaticIncludes.h>
#if OPENDDS_DO_MANUAL_STATIC_INCLUDES
#  include <dds/DCPS/RTPS/RtpsDiscovery.h>
#  include <dds/DCPS/transport/rtps_udp/RtpsUdp.h>
#endif

#include <cstdio>
#include <cstdlib>

static int interop_domain_id()
{
  const char *env = ACE_OS::getenv("AIDDS_INTEROP_DOMAIN");
  if (env != nullptr && env[0] != '\0') {
    return ACE_OS::atoi(env);
  }
  return 93;
}

static int interop_timeout_ms()
{
  const char *env = ACE_OS::getenv("AIDDS_INTEROP_TIMEOUT_MS");
  if (env != nullptr && env[0] != '\0') {
    return ACE_OS::atoi(env);
  }
  return 12000;
}

static bool try_take_sample(
  AiDdsInterop::MessageDataReader_ptr message_reader,
  InteropDataReaderListener *listener,
  CORBA::ULong expect_id,
  bool have_expect_id)
{
  AiDdsInterop::Message msg;
  DDS::SampleInfo info;
  while (message_reader->take_next_sample(msg, info) == DDS::RETCODE_OK) {
    if (!info.valid_data) {
      continue;
    }
    if (have_expect_id && msg.id != expect_id) {
      continue;
    }
    listener->set_received(msg.id, msg.payload.in());
    return true;
  }
  return false;
}

int ACE_TMAIN(int argc, ACE_TCHAR *argv[])
{
  int domain_id = interop_domain_id();
  int timeout_ms = interop_timeout_ms();
  CORBA::ULong expect_id = 0;
  bool have_expect_id = false;

  try {
    DDS::DomainParticipantFactory_var dpf = TheParticipantFactoryWithArgs(argc, argv);

    if (argc > 1) {
      expect_id = static_cast<CORBA::ULong>(ACE_OS::strtoul(ACE_TEXT_ALWAYS_CHAR(argv[1]), nullptr, 10));
      have_expect_id = true;
    }

    DDS::DomainParticipant_var participant =
      dpf->create_participant(domain_id, PARTICIPANT_QOS_DEFAULT, 0, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    if (!participant) {
      ACE_ERROR_RETURN((LM_ERROR, "create_participant failed\n"), EXIT_FAILURE);
    }

    AiDdsInterop::MessageTypeSupport_var ts = new AiDdsInterop::MessageTypeSupportImpl;
    if (ts->register_type(participant, "") != DDS::RETCODE_OK) {
      ACE_ERROR_RETURN((LM_ERROR, "register_type failed\n"), EXIT_FAILURE);
    }

    CORBA::String_var type_name = ts->get_type_name();
    DDS::Topic_var topic = participant->create_topic(
      "AiDdsInteropMessage", type_name, TOPIC_QOS_DEFAULT, 0, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    if (!topic) {
      ACE_ERROR_RETURN((LM_ERROR, "create_topic failed\n"), EXIT_FAILURE);
    }

    DDS::Subscriber_var subscriber =
      participant->create_subscriber(SUBSCRIBER_QOS_DEFAULT, 0, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    if (!subscriber) {
      ACE_ERROR_RETURN((LM_ERROR, "create_subscriber failed\n"), EXIT_FAILURE);
    }

    InteropDataReaderListener *listener_impl = new InteropDataReaderListener;
    listener_impl->set_expect_id(expect_id, have_expect_id);
    DDS::DataReaderListener_var listener(listener_impl);

    DDS::DataReaderQos dr_qos;
    subscriber->get_default_datareader_qos(dr_qos);
    dr_qos.reliability.kind = DDS::RELIABLE_RELIABILITY_QOS;

    DDS::DataReader_var reader =
      subscriber->create_datareader(topic, dr_qos, listener, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    if (!reader) {
      ACE_ERROR_RETURN((LM_ERROR, "create_datareader failed\n"), EXIT_FAILURE);
    }

    AiDdsInterop::MessageDataReader_var message_reader =
      AiDdsInterop::MessageDataReader::_narrow(reader);
    if (!message_reader) {
      ACE_ERROR_RETURN((LM_ERROR, "MessageDataReader::_narrow failed\n"), EXIT_FAILURE);
    }

    DDS::StatusCondition_var condition = reader->get_statuscondition();
    condition->set_enabled_statuses(DDS::SUBSCRIPTION_MATCHED_STATUS);
    DDS::WaitSet_var ws = new DDS::WaitSet;
    ws->attach_condition(condition);

    const int match_wait_ms = 15000;
    int match_elapsed_ms = 0;
    while (match_elapsed_ms < match_wait_ms) {
      DDS::SubscriptionMatchedStatus status;
      if (reader->get_subscription_matched_status(status) == DDS::RETCODE_OK
          && status.current_count >= 1) {
        break;
      }
      DDS::ConditionSeq conditions;
      DDS::Duration_t wait_time = {0, 100000000};
      ws->wait(conditions, wait_time);
      match_elapsed_ms += 100;
    }

    int elapsed_ms = 0;
    while (elapsed_ms < timeout_ms && !listener_impl->received()) {
      if (try_take_sample(message_reader, listener_impl, expect_id, have_expect_id)) {
        break;
      }
      ACE_OS::sleep(ACE_Time_Value(0, 50000));
      elapsed_ms += 50;
    }

    ws->detach_condition(condition);

    if (listener_impl->received()) {
      std::printf("INTEROP_RECEIVE id=%u payload=%s domain=%d\n",
                  static_cast<unsigned>(listener_impl->received_id()),
                  listener_impl->received_payload(),
                  domain_id);
      std::fflush(stdout);
      participant->delete_contained_entities();
      dpf->delete_participant(participant);
      TheServiceParticipant->shutdown();
      return EXIT_SUCCESS;
    }

    participant->delete_contained_entities();
    dpf->delete_participant(participant);
    TheServiceParticipant->shutdown();
  } catch (const CORBA::Exception &e) {
    e._tao_print_exception("Exception in interop_subscriber:");
    return EXIT_FAILURE;
  }

  std::fprintf(stderr, "INTEROP_TIMEOUT after %d ms\n", timeout_ms);
  return EXIT_FAILURE;
}
