#include "DataReaderListener.h"

#include <ace/Log_Msg.h>
#include <cstdio>

InteropDataReaderListener::InteropDataReaderListener()
  : expect_id_(0)
  , have_expect_id_(false)
  , received_(false)
  , received_id_(0)
{
}

void InteropDataReaderListener::set_expect_id(CORBA::ULong id, bool enabled)
{
  expect_id_ = id;
  have_expect_id_ = enabled;
}

void InteropDataReaderListener::set_received(CORBA::ULong id, const char *payload)
{
  received_ = true;
  received_id_ = id;
  received_payload_ = payload;
}

void InteropDataReaderListener::on_requested_deadline_missed(
  DDS::DataReader_ptr,
  const DDS::RequestedDeadlineMissedStatus &)
{
}

void InteropDataReaderListener::on_requested_incompatible_qos(
  DDS::DataReader_ptr,
  const DDS::RequestedIncompatibleQosStatus &status)
{
  ACE_ERROR((LM_ERROR, "incompatible QoS: count=%d\n", status.total_count));
}

void InteropDataReaderListener::on_sample_rejected(
  DDS::DataReader_ptr,
  const DDS::SampleRejectedStatus &status)
{
  ACE_ERROR((LM_ERROR, "sample rejected total=%d last_reason=%d\n",
             status.total_count, static_cast<int>(status.last_reason)));
}

void InteropDataReaderListener::on_liveliness_changed(
  DDS::DataReader_ptr,
  const DDS::LivelinessChangedStatus &)
{
}

void InteropDataReaderListener::on_subscription_matched(
  DDS::DataReader_ptr,
  const DDS::SubscriptionMatchedStatus &status)
{
  std::fprintf(stderr, "subscription matched current=%d total=%d\n",
               status.current_count, status.total_count);
  std::fflush(stderr);
}

void InteropDataReaderListener::on_sample_lost(
  DDS::DataReader_ptr,
  const DDS::SampleLostStatus &)
{
}

void InteropDataReaderListener::on_data_available(DDS::DataReader_ptr reader)
{
  AiDdsInterop::MessageDataReader_var message_reader =
    AiDdsInterop::MessageDataReader::_narrow(reader);
  if (!message_reader) {
    ACE_ERROR((LM_ERROR, "MessageDataReader::_narrow failed in listener\n"));
    return;
  }

  AiDdsInterop::Message msg;
  DDS::SampleInfo info;
  while (message_reader->take_next_sample(msg, info) == DDS::RETCODE_OK) {
    if (!info.valid_data) {
      continue;
    }
    if (have_expect_id_ && msg.id != expect_id_) {
      continue;
    }
    received_ = true;
    received_id_ = msg.id;
    received_payload_ = msg.payload;
    return;
  }
}
