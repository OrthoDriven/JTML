
corrosion_add_cxxbridge(direct-rs_bridge
  CRATE direct_rs
  REGEN_TARGET direct_rs_bridge_regen
  FILES src/bridge.rs
)

option(USE_RUST_DIRECT "Use Rust-based direct optimizer" 1)

target_link_libraries(direct-rs_bridge PUBLIC direct_rs jtml_domain)
target_link_libraries(jtml_coordinator PUBLIC direct-rs_bridge)
target_link_libraries(jtml_domain PUBLIC direct-rs_bridge)

target_compile_definitions(jtml_coordinator PRIVATE
  USE_RUST_DIRECT=$<BOOL:${USE_RUST_DIRECT}>)
target_compile_definitions(jtml_domain PRIVATE
  USE_RUST_DIRECT=$<BOOL:${USE_RUST_DIRECT}>)
