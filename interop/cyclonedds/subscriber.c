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

static int interop_timeout_ms(void)
{
  const char *env = getenv("AIDDS_INTEROP_TIMEOUT_MS");
  if (env != NULL && env[0] != '\0')
    return atoi(env);
  return 8000;
}

int main(int argc, char **argv)
{
  dds_entity_t participant;
  dds_entity_t topic;
  dds_entity_t reader;
  dds_return_t rc;
  AiDdsInterop_Message msg;
  void *samples[1];
  dds_sample_info_t infos[1];
  int domain_id = interop_domain_id();
  int timeout_ms = interop_timeout_ms();
  int elapsed = 0;
  unsigned long expect_id = 0;
  int have_expect_id = 0;

  (void)argc;
  if (argc > 1)
  {
    expect_id = (unsigned long) strtoul(argv[1], NULL, 10);
    have_expect_id = 1;
  }

  participant = dds_create_participant(domain_id, NULL, NULL);
  if (participant < 0)
    DDS_FATAL("dds_create_participant: %s\n", dds_strretcode(-participant));

  topic = dds_create_topic(
    participant, &AiDdsInterop_Message_desc, "AiDdsInteropMessage", NULL, NULL);
  if (topic < 0)
    DDS_FATAL("dds_create_topic: %s\n", dds_strretcode(-topic));

  reader = dds_create_reader(participant, topic, NULL, NULL);
  if (reader < 0)
    DDS_FATAL("dds_create_reader: %s\n", dds_strretcode(-reader));

  samples[0] = AiDdsInterop_Message__alloc();

  while (elapsed < timeout_ms)
  {
    rc = dds_take(reader, samples, infos, 1, 1);
    if (rc > 0 && infos[0].valid_data)
    {
      msg = *(AiDdsInterop_Message *) samples[0];
      if (have_expect_id && msg.id != expect_id)
      {
        printf("INTEROP_RECEIVE id=%u payload=%s (ignored, expected %lu)\n",
               msg.id, msg.payload, expect_id);
      }
      else
      {
        printf("INTEROP_RECEIVE id=%u payload=%s domain=%d\n",
               msg.id, msg.payload, domain_id);
        fflush(stdout);
        AiDdsInterop_Message_free(samples[0], DDS_FREE_ALL);
        dds_delete(participant);
        return EXIT_SUCCESS;
      }
    }
    dds_sleepfor(DDS_MSECS(20));
    elapsed += 20;
  }

  AiDdsInterop_Message_free(samples[0], DDS_FREE_ALL);
  dds_delete(participant);
  fprintf(stderr, "INTEROP_TIMEOUT after %d ms\n", timeout_ms);
  return EXIT_FAILURE;
}
