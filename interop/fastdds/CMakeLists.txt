cmake_minimum_required(VERSION 3.16)
project(aidds_interop_fastdds LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 11)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(fastcdr REQUIRED)
find_package(fastrtps REQUIRED)

if(DEFINED ENV{FASTDDSGEN})
  set(FASTDDSGEN "$ENV{FASTDDSGEN}")
else()
  find_program(FASTDDSGEN fastddsgen)
endif()

if(NOT FASTDDSGEN)
  message(FATAL_ERROR "fastddsgen not found. Set FASTDDSGEN or install Fast DDS-Gen (v3.x for Fast DDS 2.14).")
endif()

set(GEN_DIR "${CMAKE_CURRENT_BINARY_DIR}/generated")
file(MAKE_DIRECTORY "${GEN_DIR}")

set(INTEROP_IDL "${CMAKE_CURRENT_SOURCE_DIR}/../idl/InteropMessage.idl")
set(SHAPES_IDL "${CMAKE_CURRENT_SOURCE_DIR}/../idl/ShapeType.idl")

set(GEN_MESSAGE_H "${GEN_DIR}/InteropMessage.h")
set(GEN_MESSAGE_CXX "${GEN_DIR}/InteropMessage.cxx")
set(GEN_PUBSUB_H "${GEN_DIR}/InteropMessagePubSubTypes.h")
set(GEN_PUBSUB_CXX "${GEN_DIR}/InteropMessagePubSubTypes.cxx")

set(GEN_SHAPES_H "${GEN_DIR}/ShapeType.h")
set(GEN_SHAPES_CXX "${GEN_DIR}/ShapeType.cxx")
set(GEN_SHAPES_PUBSUB_H "${GEN_DIR}/ShapeTypePubSubTypes.h")
set(GEN_SHAPES_PUBSUB_CXX "${GEN_DIR}/ShapeTypePubSubTypes.cxx")

add_custom_command(
  OUTPUT "${GEN_MESSAGE_H}" "${GEN_MESSAGE_CXX}" "${GEN_PUBSUB_H}" "${GEN_PUBSUB_CXX}"
  COMMAND "${FASTDDSGEN}" -replace -d "${GEN_DIR}" "${INTEROP_IDL}"
  DEPENDS "${INTEROP_IDL}"
  COMMENT "Generating Fast DDS types from InteropMessage.idl"
)

add_custom_command(
  OUTPUT "${GEN_SHAPES_H}" "${GEN_SHAPES_CXX}" "${GEN_SHAPES_PUBSUB_H}" "${GEN_SHAPES_PUBSUB_CXX}"
  COMMAND "${FASTDDSGEN}" -replace -d "${GEN_DIR}" "${SHAPES_IDL}"
  DEPENDS "${SHAPES_IDL}"
  COMMENT "Generating Fast DDS types from ShapeType.idl"
)

add_executable(interop_publisher publisher.cpp "${GEN_MESSAGE_CXX}" "${GEN_PUBSUB_CXX}")
add_executable(interop_subscriber subscriber.cpp "${GEN_MESSAGE_CXX}" "${GEN_PUBSUB_CXX}")
add_executable(shapes_publisher shapes_publisher.cpp "${GEN_SHAPES_CXX}" "${GEN_SHAPES_PUBSUB_CXX}")
add_executable(shapes_subscriber shapes_subscriber.cpp "${GEN_SHAPES_CXX}" "${GEN_SHAPES_PUBSUB_CXX}")

add_custom_target(fastdds_idl ALL DEPENDS
  "${GEN_MESSAGE_H}" "${GEN_MESSAGE_CXX}" "${GEN_PUBSUB_H}" "${GEN_PUBSUB_CXX}")
add_custom_target(fastdds_shapes_idl ALL DEPENDS
  "${GEN_SHAPES_H}" "${GEN_SHAPES_CXX}" "${GEN_SHAPES_PUBSUB_H}" "${GEN_SHAPES_PUBSUB_CXX}")
add_dependencies(interop_publisher fastdds_idl)
add_dependencies(interop_subscriber fastdds_idl)
add_dependencies(shapes_publisher fastdds_shapes_idl)
add_dependencies(shapes_subscriber fastdds_shapes_idl)

set_source_files_properties(publisher.cpp subscriber.cpp PROPERTIES
  OBJECT_DEPENDS "${GEN_MESSAGE_H};${GEN_PUBSUB_H}")
set_source_files_properties(shapes_publisher.cpp shapes_subscriber.cpp PROPERTIES
  OBJECT_DEPENDS "${GEN_SHAPES_H};${GEN_SHAPES_PUBSUB_H}")

foreach(target interop_publisher interop_subscriber shapes_publisher shapes_subscriber)
  target_include_directories(${target} PRIVATE "${GEN_DIR}")
  target_compile_definitions(${target} PRIVATE
    AIDDS_FASTDDS_INTEROP_PROFILE="${CMAKE_CURRENT_SOURCE_DIR}/aidds_interop_profile.xml")
  target_link_libraries(${target} PRIVATE fastrtps fastcdr)
endforeach()
