
corrosion_add_cxxbridge(nfd-rs_bridge
  CRATE nfd
  REGEN_TARGET nfd_bridge_regen
  FILES src/bridge.rs
)

target_link_libraries(
    nfd-rs_bridge
    PUBLIC
        nfd
)
