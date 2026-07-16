cmake_minimum_required(VERSION 3.16)
project(aidds_interop_opensplice LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 11)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(NOT DEFINED ENV{OSPL_HOME})
  message(FATAL_ERROR "OSPL_HOME is not set. Install OpenSplice/Vortex OpenSplice and export OSPL_HOME.")
endif()

set(OSPL_HOME "$ENV{OSPL_HOME}")
set(IDL_FILE "${CMAKE_CURRENT_SOURCE_DIR}/../idl/InteropMessage.idl")
set(GEN_DIR "${CMAKE_CURRENT_BINARY_DIR}/generated")
file(MAKE_DIRECTORY "${GEN_DIR}")

if(EXISTS "${OSPL_HOME}/release.com")
  set(OSPL_IDLPP "${OSPL_HOME}/bin/idlpp")
else()
  set(OSPL_IDLPP "${OSPL_HOME}/bin/idlpp")
endif()

if(NOT EXISTS "${OSPL_IDLPP}")
  message(FATAL_ERROR "OpenSplice idlpp not found at ${OSPL_IDLPP}")
endif()

set(GEN_STAMP "${GEN_DIR}/generated.stamp")
add_custom_command(
  OUTPUT "${GEN_STAMP}"
  COMMAND "${OSPL_IDLPP}" -S -l isocpp2 -d "${GEN_DIR}" "${IDL_FILE}"
  COMMAND "${CMAKE_COMMAND}" -E touch "${GEN_STAMP}"
  DEPENDS "${IDL_FILE}"
  COMMENT "Generating OpenSplice isocpp2 types from InteropMessage.idl"
  WORKING_DIRECTORY "${GEN_DIR}"
)

add_custom_target(opensplice_idl DEPENDS "${GEN_STAMP}")

file(GLOB_RECURSE OPENSPLICE_GEN_SOURCES CONFIGURE_DEPENDS
  "${GEN_DIR}/*.cpp"
  "${GEN_DIR}/*.cxx")

include_directories(
  "${OSPL_HOME}/include"
  "${OSPL_HOME}/include/sys"
  "${OSPL_HOME}/include/dcps/C++/isocpp2"
  "${GEN_DIR}"
)

link_directories("${OSPL_HOME}/lib")

foreach(lib ddskernel ddsuser isocpp2 dcpsisocpp dcpssacpp)
  find_library(OSPL_${lib} NAMES ${lib} PATHS "${OSPL_HOME}/lib")
endforeach()

set(OSPL_LIBS)
foreach(lib ddskernel ddsuser isocpp2 dcpsisocpp dcpssacpp)
  if(OSPL_${lib})
    list(APPEND OSPL_LIBS ${OSPL_${lib}})
  endif()
endforeach()

if(NOT OSPL_LIBS)
  message(FATAL_ERROR "No OpenSplice libraries found under ${OSPL_HOME}/lib")
endif()

add_executable(interop_publisher publisher.cpp ${OPENSPLICE_GEN_SOURCES})
add_executable(interop_subscriber subscriber.cpp ${OPENSPLICE_GEN_SOURCES})

add_dependencies(interop_publisher opensplice_idl)
add_dependencies(interop_subscriber opensplice_idl)

foreach(target interop_publisher interop_subscriber)
  target_link_libraries(${target} PRIVATE ${OSPL_LIBS} pthread dl)
endforeach()
