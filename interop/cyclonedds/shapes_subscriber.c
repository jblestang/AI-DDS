#include "dds/dds.h"
#include "ShapeType.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int shapes_domain_id(void)
{
  const char *env = getenv("AIDDS_SHAPES_DOMAIN");
  if (env != NULL && env[0] != '\0')
    return atoi(env);
  return 120;
}

static int shapes_timeout_ms(void)
{
  const char *env = getenv("AIDDS_SHAPES_TIMEOUT_MS");
  if (env != NULL && env[0] != '\0')
    return atoi(env);
  return 12000;
}

static const char *default_topic(int argc, char **argv)
{
  if (argc > 1 && argv[1][0] != '\0')
    return argv[1];
  return "Square";
}

int main(int argc, char **argv)
{
  dds_entity_t participant;
  dds_entity_t topic;
  dds_entity_t reader;
  dds_return_t rc;
  org_omg_dds_demo_ShapeType msg;
  void *samples[1];
  dds_sample_info_t infos[1];
  int domain_id = shapes_domain_id();
  int timeout_ms = shapes_timeout_ms();
  int elapsed = 0;
  const char *topic_name = default_topic(argc, argv);
  const char *expect_color = NULL;

  if (argc > 2)
    expect_color = argv[2];

  participant = dds_create_participant(domain_id, NULL, NULL);
  if (participant < 0)
    DDS_FATAL("dds_create_participant: %s\n", dds_strretcode(-participant));

  topic = dds_create_topic(
    participant, &org_omg_dds_demo_ShapeType_desc, topic_name, NULL, NULL);
  if (topic < 0)
    DDS_FATAL("dds_create_topic: %s\n", dds_strretcode(-topic));

  reader = dds_create_reader(participant, topic, NULL, NULL);
  if (reader < 0)
    DDS_FATAL("dds_create_reader: %s\n", dds_strretcode(-reader));

  samples[0] = org_omg_dds_demo_ShapeType__alloc();

  while (elapsed < timeout_ms)
  {
    rc = dds_take(reader, samples, infos, 1, 1);
    if (rc > 0 && infos[0].valid_data)
    {
      msg = *(org_omg_dds_demo_ShapeType *) samples[0];
      if (expect_color != NULL && strcmp(msg.color, expect_color) != 0)
      {
        printf(
          "SHAPES_RECEIVE topic=%s color=%s x=%d y=%d shapesize=%d (ignored, expected %s)\n",
          topic_name, msg.color, (int) msg.x, (int) msg.y, (int) msg.shapesize, expect_color);
      }
      else
      {
        printf(
          "SHAPES_RECEIVE topic=%s color=%s x=%d y=%d shapesize=%d domain=%d\n",
          topic_name, msg.color, (int) msg.x, (int) msg.y, (int) msg.shapesize, domain_id);
        fflush(stdout);
        org_omg_dds_demo_ShapeType_free(samples[0], DDS_FREE_ALL);
        dds_delete(participant);
        return EXIT_SUCCESS;
      }
    }
    dds_sleepfor(DDS_MSECS(20));
    elapsed += 20;
  }

  org_omg_dds_demo_ShapeType_free(samples[0], DDS_FREE_ALL);
  dds_delete(participant);
  fprintf(stderr, "SHAPES_TIMEOUT after %d ms\n", timeout_ms);
  return EXIT_FAILURE;
}
