#ifndef AIDDS_INTEROP_DATA_READER_LISTENER_H
#define AIDDS_INTEROP_DATA_READER_LISTENER_H

#include "InteropMessageTypeSupportImpl.h"

#include <dds/DdsDcpsSubscriptionC.h>
#include <dds/DCPS/LocalObject.h>

class InteropDataReaderListener
  : public virtual OpenDDS::DCPS::LocalObject<DDS::DataReaderListener> {
public:
  InteropDataReaderListener();

  void on_data_available(DDS::DataReader_ptr reader) override;

  void on_requested_deadline_missed(
    DDS::DataReader_ptr reader,
    const DDS::RequestedDeadlineMissedStatus &status) override;

  void on_requested_incompatible_qos(
    DDS::DataReader_ptr reader,
    const DDS::RequestedIncompatibleQosStatus &status) override;

  void on_sample_rejected(
    DDS::DataReader_ptr reader,
    const DDS::SampleRejectedStatus &status) override;

  void on_liveliness_changed(
    DDS::DataReader_ptr reader,
    const DDS::LivelinessChangedStatus &status) override;

  void on_subscription_matched(
    DDS::DataReader_ptr reader,
    const DDS::SubscriptionMatchedStatus &status) override;

  void on_sample_lost(
    DDS::DataReader_ptr reader,
    const DDS::SampleLostStatus &status) override;

  void set_expect_id(CORBA::ULong id, bool enabled);

  bool received() const { return received_; }

  CORBA::ULong received_id() const { return received_id_; }

  const char *received_payload() const { return received_payload_.in(); }

private:
  CORBA::ULong expect_id_;
  bool have_expect_id_;
  bool received_;
  CORBA::ULong received_id_;
  CORBA::String_var received_payload_;
};

#endif
