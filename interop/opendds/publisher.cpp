/*
 * OpenDDS interop publisher for AI-DDS harness (InteropMessage.idl).
 */

#include "InteropMessageTypeSupportImpl.h"

#include <ace/Log_Msg.h>
#include <ace/OS_NS_stdlib.h>
#include <ace/OS_NS_unistd.h>

#include <dds/DdsDcpsInfrastructureC.h>
#include <dds/DdsDcpsPublicationC.h>
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
#include <cstring>

static int interop_domain_id()
{
  const char *env = ACE_OS::getenv("AIDDS_INTEROP_DOMAIN");
  if (env != nullptr && env[0] != '\0') {
    return ACE_OS::atoi(env);
  }
  return 93;
}

static int interop_wait_match()
{
  const char *env = ACE_OS::getenv("AIDDS_INTEROP_WAIT_MATCH");
  if (env != nullptr && env[0] != '\0') {
    return ACE_OS::atoi(env);
  }
  return 1;
}

static void wait_for_publication_match(DDS::DataWriter_var writer)
{
  DDS::StatusCondition_var condition = writer->get_statuscondition();
  condition->set_enabled_statuses(DDS::PUBLICATION_MATCHED_STATUS);

  DDS::WaitSet_var ws = new DDS::WaitSet;
  ws->attach_condition(condition);

  const int max_wait_secs = 30;
  int waited_secs = 0;
  while (waited_secs < max_wait_secs) {
    DDS::PublicationMatchedStatus matches;
    if (writer->get_publication_matched_status(matches) != DDS::RETCODE_OK) {
      ACE_ERROR((LM_ERROR, "get_publication_matched_status failed\n"));
      ws->detach_condition(condition);
      return;
    }
    if (matches.current_count >= 1) {
      ws->detach_condition(condition);
      return;
    }
    DDS::ConditionSeq conditions;
    DDS::Duration_t timeout = {1, 0};
    if (ws->wait(conditions, timeout) != DDS::RETCODE_OK) {
      ++waited_secs;
      continue;
    }
  }

  ACE_ERROR((LM_ERROR, "wait for publication match timed out after %d s\n", max_wait_secs));
  ws->detach_condition(condition);
}

int ACE_TMAIN(int argc, ACE_TCHAR *argv[])
{
  int domain_id = interop_domain_id();
  CORBA::ULong sample_id = 4242;
  const char *payload = "opendds-to-aidds";

  try {
    DDS::DomainParticipantFactory_var dpf = TheParticipantFactoryWithArgs(argc, argv);

    if (argc > 1) {
      sample_id = static_cast<CORBA::ULong>(ACE_OS::strtoul(ACE_TEXT_ALWAYS_CHAR(argv[1]), nullptr, 10));
    }
    if (argc > 2) {
      payload = ACE_TEXT_ALWAYS_CHAR(argv[2]);
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

    DDS::Publisher_var publisher =
      participant->create_publisher(PUBLISHER_QOS_DEFAULT, 0, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    if (!publisher) {
      ACE_ERROR_RETURN((LM_ERROR, "create_publisher failed\n"), EXIT_FAILURE);
    }

    DDS::DataWriter_var writer =
      publisher->create_datawriter(topic, DATAWRITER_QOS_DEFAULT, 0, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    if (!writer) {
      ACE_ERROR_RETURN((LM_ERROR, "create_datawriter failed\n"), EXIT_FAILURE);
    }

    AiDdsInterop::MessageDataWriter_var message_writer =
      AiDdsInterop::MessageDataWriter::_narrow(writer);
    if (!message_writer) {
      ACE_ERROR_RETURN((LM_ERROR, "MessageDataWriter::_narrow failed\n"), EXIT_FAILURE);
    }

    if (interop_wait_match()) {
      wait_for_publication_match(writer);
      DDS::PublicationMatchedStatus matches;
      if (writer->get_publication_matched_status(matches) != DDS::RETCODE_OK
          || matches.current_count < 1) {
        ACE_ERROR_RETURN((LM_ERROR, "no publication match; refusing to write\n"), EXIT_FAILURE);
      }
    }

    AiDdsInterop::Message msg;
    msg.id = sample_id;
    msg.payload = payload;

    std::printf("INTEROP_PUBLISH id=%lu payload=%s domain=%d\n",
                static_cast<unsigned long>(sample_id), payload, domain_id);
    std::fflush(stdout);

    if (message_writer->write(msg, DDS::HANDLE_NIL) != DDS::RETCODE_OK) {
      ACE_ERROR_RETURN((LM_ERROR, "write failed\n"), EXIT_FAILURE);
    }

    DDS::Duration_t ack_timeout = {5, 0};
    message_writer->wait_for_acknowledgments(ack_timeout);

    ACE_OS::sleep(2);

    participant->delete_contained_entities();
    dpf->delete_participant(participant);
    TheServiceParticipant->shutdown();
  } catch (const CORBA::Exception &e) {
    e._tao_print_exception("Exception in interop_publisher:");
    return EXIT_FAILURE;
  }

  return EXIT_SUCCESS;
}
