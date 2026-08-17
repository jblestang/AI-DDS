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

static int shapes_wait_match(void)
{
  const char *env = getenv("AIDDS_SHAPES_WAIT_MATCH");
  if (env != NULL && env[0] != '\0')
    return atoi(env);
  return 1;
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
  dds_entity_t writer;
  dds_return_t rc;
  org_omg_dds_demo_ShapeType msg;
  uint32_t status = 0;
  int domain_id = shapes_domain_id();
  const char *topic_name = default_topic(argc, argv);
  const char *color = "RED";
  int32_t x = 10;
  int32_t y = 20;
  int32_t shapesize = 30;

  if (argc > 2)
    color = argv[2];
  if (argc > 3)
    x = (int32_t) strtol(argv[3], NULL, 10);
  if (argc > 4)
    y = (int32_t) strtol(argv[4], NULL, 10);
  if (argc > 5)
    shapesize = (int32_t) strtol(argv[5], NULL, 10);

  participant = dds_create_participant(domain_id, NULL, NULL);
  if (participant < 0)
    DDS_FATAL("dds_create_participant: %s\n", dds_strretcode(-participant));

  topic = dds_create_topic(
    participant, &org_omg_dds_demo_ShapeType_desc, topic_name, NULL, NULL);
  if (topic < 0)
    DDS_FATAL("dds_create_topic: %s\n", dds_strretcode(-topic));

  writer = dds_create_writer(participant, topic, NULL, NULL);
  if (writer < 0)
    DDS_FATAL("dds_create_writer: %s\n", dds_strretcode(-writer));

  if (shapes_wait_match())
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

  msg.color = (char *) color;
  msg.x = x;
  msg.y = y;
  msg.shapesize = shapesize;

  printf(
    "SHAPES_PUBLISH topic=%s color=%s x=%d y=%d shapesize=%d domain=%d\n",
    topic_name, msg.color, (int) msg.x, (int) msg.y, (int) msg.shapesize, domain_id);
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
