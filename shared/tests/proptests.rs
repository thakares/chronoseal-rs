use proptest::prelude::*;

proptest! {
    #[test]
    fn test_vm_execute_never_panics(ref program in any::<Vec<u8>>()) {
        // VM execution should be totally robust and never panic on any random input stream.
        let state = shared::vm::execute(program);
        // The instruction pointer (ip) should not exceed the program length
        assert!(state.ip as usize <= program.len());
    }

    #[test]
    fn test_gene_environment_roundtrip_never_panics(ref data in any::<Vec<u8>>()) {
        // Try to decode random bytes. It should either succeed or fail gracefully, never panic.
        let _ = shared::gene::decode_environment(data);
    }
}
