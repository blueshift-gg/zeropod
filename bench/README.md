# Benchmarks

`cargo bench -p bench`: the same account-like value (`src/lib.rs`) in borsh,
wincode and zeropod, each reading bytes it wrote.

- `encode`: into a buffer that exists, so the system allocator is not measured.
- `decode`: into an owned value.
- `read_last_field`: from bytes to the vector after a string; wincode borrows
  (`AccountRef`), its fastest read, and borsh decodes everything.
- `update_fixed_field`, `update_variable_field`: leave the bytes encoding the
  changed value. borsh and wincode decode, change and re-encode; zeropod
  writes the field in place, and for the string moves the field after it.

Apple M-series, Rust 1.92, criterion medians:

| | borsh | wincode | zeropod |
|---|---:|---:|---:|
| encode | 36.9 ns | 6.6 ns | 8.8 ns |
| decode | 75.1 ns | 51.7 ns | 51.6 ns |
| read_last_field | 74.4 ns | 4.2 ns | 6.1 ns |
| update_fixed_field | 114.5 ns | 57.9 ns | 6.7 ns |
| update_variable_field | 134.7 ns | 75.0 ns | 14.7 ns |
