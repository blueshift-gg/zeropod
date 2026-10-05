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
| encode | 40.9 ns | 6.2 ns | 6.9 ns |
| decode | 76.6 ns | 52.5 ns | 51.8 ns |
| read_last_field | 77.4 ns | 4.4 ns | 5.5 ns |
| update_fixed_field | 118.2 ns | 58.7 ns | 5.9 ns |
| update_variable_field | 136.3 ns | 77.1 ns | 14.0 ns |

| | bytemuck | zerocopy | zeropod |
|---|---:|---:|---:|
| fixed_read | 0.41 ns | 0.40 ns | 0.38 ns |
| fixed_update | 1.07 ns | 0.99 ns | 0.75 ns |

Below a nanosecond the loop decides the number: compiled on their own, the
bytemuck and zeropod reads are the same four instructions, a length compare
and a load. wincode reads one field faster from fresh bytes because it
borrows the fields in one pass; a zeropod view validates once, then finds a
field's offset when it is read, which is what lets it write in place.
