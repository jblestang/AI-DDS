cmake_minimum_required(VERSION 3.16)
project(aidds_interop_cyclonedds LANGUAGES C)

if(NOT TARGET CycloneDDS::ddsc)
  find_package(CycloneDDS REQUIRED)
endif()

idlc_generate(TARGET InteropMessage_lib FILES "../idl/InteropMessage.idl" WARNINGS no-implicit-extensibility)

add_executable(interop_publisher publisher.c)
add_executable(interop_subscriber subscriber.c)

target_link_libraries(interop_publisher InteropMessage_lib CycloneDDS::ddsc)
target_link_libraries(interop_subscriber InteropMessage_lib CycloneDDS::ddsc)
