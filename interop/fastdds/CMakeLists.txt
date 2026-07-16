cmake_minimum_required(VERSION 3.16)
project(aidds_interop_fastdds LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 11)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(fastcdr REQUIRED)
find_package(fastdds REQUIRED)

if(DEFINED ENV{FASTDDSGEN})
  set(FASTDDSGEN "$ENV{FASTDDSGEN}")
else()
  find_program(FASTDDSGEN fastddsgen)
endif()

if(NOT FASTDDSGEN)
  message(FATAL_ERROR "fastddsgen not found. Set FASTDDSGEN or install Fast DDS-Gen.")
endif()

set(IDL_FILE "${CMAKE_CURRENT_SOURCE_DIR}/../idl/InteropMessage.idl")
set(GEN_DIR "${CMAKE_CURRENT_BINARY_DIR}/generated")
file(MAKE_DIRECTORY "${GEN_DIR}")

set(GEN_TYPE_SUPPORT "${GEN_DIR}/InteropMessagePubSubTypes.cxx")
set(GEN_TYPE "${GEN_DIR}/InteropMessage.cxx")
set(GEN_TYPE_HEADER "${GEN_DIR}/InteropMessage.h")
set(GEN_PUBSUB_HEADER "${GEN_DIR}/InteropMessagePubSubTypes.hpp")

add_custom_command(
  OUTPUT "${GEN_TYPE}" "${GEN_TYPE_SUPPORT}" "${GEN_TYPE_HEADER}" "${GEN_PUBSUB_HEADER}"
  COMMAND "${FASTDDSGEN}" -replace -d "${GEN_DIR}" "${IDL_FILE}"
  DEPENDS "${IDL_FILE}"
  COMMENT "Generating Fast DDS types from InteropMessage.idl"
)

add_executable(interop_publisher publisher.cpp "${GEN_TYPE}" "${GEN_TYPE_SUPPORT}")
add_executable(interop_subscriber subscriber.cpp "${GEN_TYPE}" "${GEN_TYPE_SUPPORT}")

foreach(target interop_publisher interop_subscriber)
  target_include_directories(${target} PRIVATE "${GEN_DIR}")
  target_link_libraries(${target} PRIVATE fastdds fastcdr)
endforeach()
