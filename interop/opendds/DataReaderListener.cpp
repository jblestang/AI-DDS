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
  const DDS::SampleRejectedStatus &)
{
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
  ACE_DEBUG((LM_DEBUG, "subscription matched current=%d total=%d\n",
             status.current_count, status.total_count));
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
