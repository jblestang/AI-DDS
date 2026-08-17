cmake_minimum_required(VERSION 3.16)
project(aidds_interop_opendds CXX)

set(CMAKE_CXX_STANDARD 14)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(OpenDDS REQUIRED)

get_filename_component(_OPENDDS_INSTALL_PREFIX "${OpenDDS_DIR}/../../.." ABSOLUTE)

if(NOT OPENDDS_BUILD_DIR)
  set(_opendds_build_guess "$ENV{OPENDDS_SRC}/build")
  if(EXISTS "${_opendds_build_guess}/opendds_config_include")
    set(OPENDDS_BUILD_DIR "${_opendds_build_guess}" CACHE PATH "OpenDDS build tree (generated headers)")
  endif()
endif()

if(NOT OPENDDS_SOURCE_DIR)
  set(OPENDDS_SOURCE_DIR "$ENV{OPENDDS_SRC}" CACHE PATH "OpenDDS source tree")
endif()

function(aidds_add_opendds_build_includes target)
  if(OPENDDS_BUILD_DIR)
    target_include_directories(${target} PRIVATE
      "${OPENDDS_SOURCE_DIR}"
      "${OPENDDS_BUILD_DIR}/_deps/rapidjson-src/include"
      "${OPENDDS_BUILD_DIR}/dds/opendds_generated"
      "${OPENDDS_BUILD_DIR}/opendds_config_include"
      "${OPENDDS_BUILD_DIR}/dds/opendds_generated/dds"
      "${OPENDDS_BUILD_DIR}/dds/DCPS/RTPS/opendds_generated"
      "${OPENDDS_BUILD_DIR}/dds/DCPS/RTPS/opendds_generated/dds/DCPS/RTPS"
    )
  endif()
endfunction()

add_library(InteropMessage_idl)
opendds_target_sources(InteropMessage_idl PUBLIC
  "${CMAKE_CURRENT_SOURCE_DIR}/../idl/InteropMessage.idl"
  TAO_IDL_OPTIONS "-I${_OPENDDS_INSTALL_PREFIX}/include"
)
target_link_libraries(InteropMessage_idl PUBLIC OpenDDS::Dcps)
aidds_add_opendds_build_includes(InteropMessage_idl)

set(opendds_libs
  OpenDDS::Dcps
  OpenDDS::Rtps
  OpenDDS::Rtps_Udp
  InteropMessage_idl
)

add_executable(interop_publisher publisher.cpp)
add_executable(interop_subscriber subscriber.cpp DataReaderListener.cpp)

target_link_libraries(interop_publisher ${opendds_libs})
target_link_libraries(interop_subscriber ${opendds_libs})
aidds_add_opendds_build_includes(interop_publisher)
aidds_add_opendds_build_includes(interop_subscriber)

configure_file("${CMAKE_CURRENT_SOURCE_DIR}/rtps.ini" "${CMAKE_CURRENT_BINARY_DIR}/rtps.ini" COPYONLY)
