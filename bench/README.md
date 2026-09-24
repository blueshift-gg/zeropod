# Benchmarks

`cargo bench -p bench`: the same account-like value (`src/lib.rs`) in borsh,
wincode and zeropod, each reading bytes it wrote; and its fixed fields alone
in bytemuck, zerocopy and zeropod.

- `encode`: into a buffer that exists, so the system allocator is not measured.
- `decode`: into an owned value.
- `read_last_field`: from bytes to the vector after a string; wincode borrows
  (`AccountRef`), its fastest read, and borsh decodes everything.
- `update_fixed_field`, `update_variable_field`: leave the bytes encoding the
  changed value. borsh and wincode decode, change and re-encode; zeropod
  writes the field in place, and for the string moves the field after it.
- `fixed_read`, `fixed_update`: bytemuck and zerocopy cast the bytes to a
  struct; zeropod views them. Each checks the length, then reads the amount
  or adds one to it.

Apple M-series, Rust 1.92, criterion medians:

| | borsh | wincode | zeropod |
|---|---:|---:|---:|
| encode | 45.3 ns | 6.5 ns | 6.7 ns |
| decode | 81.7 ns | 56.9 ns | 58.8 ns |
| read_last_field | 79.6 ns | 4.7 ns | 5.9 ns |
| update_fixed_field | 129.1 ns | 61.7 ns | 6.6 ns |
| update_variable_field | 143.0 ns | 87.6 ns | 14.3 ns |

| | bytemuck | zerocopy | zeropod |
|---|---:|---:|---:|
| fixed_read | 0.38 ns | 0.42 ns | 0.78 ns |
| fixed_update | 1.13 ns | 1.01 ns | 1.28 ns |

Below a nanosecond the loop decides the number: compiled on their own, the
bytemuck and zeropod reads are the same four instructions, a length compare
and a load. wincode reads one field faster from fresh bytes because it
borrows the fields in one pass; a zeropod view validates once, then finds a
field's offset when it is read, which is what lets it write in place.
