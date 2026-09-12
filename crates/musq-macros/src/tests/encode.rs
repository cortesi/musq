derive_expansion_tests!(
    encode::expand_derive_encode,
    enum_impl = "impl :: musq :: encode :: Encode for Foo",
    enum_generic_impl = "impl < T > :: musq :: encode :: Encode for Foo < T >",
    struct_impl = "impl :: musq :: encode :: Encode for Foo",
    struct_body = "self . 0",
);
