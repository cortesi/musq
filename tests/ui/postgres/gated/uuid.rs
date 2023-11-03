fn main() {
    let _ = query!("select 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11'::uuid");
    let _ = query!("select $1::uuid", ());
}
