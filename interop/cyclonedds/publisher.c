#include "dds/dds.h"
#include "InteropMessage.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int interop_domain_id(void)
{
  const char *env = getenv("AIDDS_INTEROP_DOMAIN");
  if (env != NULL && env[0] != '\0')
    return atoi(env);
  return 70;
}

static int interop_wait_match(void)
{
  const char *env = getenv("AIDDS_INTEROP_WAIT_MATCH");
  if (env != NULL && env[0] != '\0')
    return atoi(env);
  return 1;
}

int main(int argc, char **argv)
{
  dds_entity_t participant;
  dds_entity_t topic;
  dds_entity_t writer;
  dds_return_t rc;
  AiDdsInterop_Message msg;
  uint32_t status = 0;
  int domain_id = interop_domain_id();
  unsigned long sample_id = 4242;
  const char *payload = "cyclonedds-to-aidds";

  if (argc > 1)
    sample_id = (unsigned long) strtoul(argv[1], NULL, 10);
  if (argc > 2)
    payload = argv[2];

  participant = dds_create_participant(domain_id, NULL, NULL);
  if (participant < 0)
    DDS_FATAL("dds_create_participant: %s\n", dds_strretcode(-participant));

  topic = dds_create_topic(
    participant, &AiDdsInterop_Message_desc, "AiDdsInteropMessage", NULL, NULL);
  if (topic < 0)
    DDS_FATAL("dds_create_topic: %s\n", dds_strretcode(-topic));

  writer = dds_create_writer(participant, topic, NULL, NULL);
  if (writer < 0)
    DDS_FATAL("dds_create_writer: %s\n", dds_strretcode(-writer));

  if (interop_wait_match())
  {
    rc = dds_set_status_mask(writer, DDS_PUBLICATION_MATCHED_STATUS);
    if (rc != DDS_RETCODE_OK)
      DDS_FATAL("dds_set_status_mask: %s\n", dds_strretcode(-rc));

    while (!(status & DDS_PUBLICATION_MATCHED_STATUS))
    {
      rc = dds_get_status_changes(writer, &status);
      if (rc != DDS_RETCODE_OK)
        DDS_FATAL("dds_get_status_changes: %s\n", dds_strretcode(-rc));
      dds_sleepfor(DDS_MSECS(20));
    }
  }

  msg.id = sample_id;
  msg.payload = (char *) payload;

  printf("INTEROP_PUBLISH id=%lu payload=%s domain=%d\n", msg.id, msg.payload, domain_id);
  fflush(stdout);

  rc = dds_write(writer, &msg);
  if (rc != DDS_RETCODE_OK)
    DDS_FATAL("dds_write: %s\n", dds_strretcode(-rc));

  dds_sleepfor(DDS_MSECS(2000));
  rc = dds_delete(participant);
  if (rc != DDS_RETCODE_OK)
    DDS_FATAL("dds_delete: %s\n", dds_strretcode(-rc));

  return EXIT_SUCCESS;
}
