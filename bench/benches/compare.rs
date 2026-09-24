//! Each group does one job three ways, from bytes each library wrote.

use std::hint::black_box;

use bench::{Account, AccountRef, account};
use borsh::BorshDeserialize;
use criterion::{Criterion, criterion_group, criterion_main};
use zeropod::Layout;

/// Writing into a buffer that exists: the encoding alone.
fn encode(c: &mut Criterion) {
    let value = account();
    let mut bytes = vec![0; 1024];
    let mut group = c.benchmark_group("encode");
    group.bench_function("borsh", |b| {
        b.iter(|| borsh::to_writer(&mut bytes[..], black_box(&value)).unwrap())
    });
    group.bench_function("wincode", |b| {
        b.iter(|| wincode::serialize_into(&mut bytes[..], black_box(&value)).unwrap())
    });
    group.bench_function("zeropod", |b| {
        b.iter(|| zeropod::write(black_box(&value), &mut bytes).unwrap())
    });
    group.finish();
}

fn decode(c: &mut Criterion) {
    let (borsh, wincode) = (
        borsh::to_vec(&account()).unwrap(),
        wincode::serialize(&account()).unwrap(),
    );
    let mut group = c.benchmark_group("decode");
    group.bench_function("borsh", |b| {
        b.iter(|| Account::try_from_slice(black_box(&borsh)).unwrap())
    });
    group.bench_function("wincode", |b| {
        b.iter(|| wincode::deserialize::<Account>(black_box(&wincode)).unwrap())
    });
    group.bench_function("zeropod", |b| {
        b.iter(|| zeropod::from_slice::<Account>(black_box(&borsh)).unwrap())
    });
    group.finish();
}

/// Reading the last field: everything before it is decoded, borrowed or
/// walked, and checked.
fn read_last_field(c: &mut Criterion) {
    let (borsh, wincode) = (
        borsh::to_vec(&account()).unwrap(),
        wincode::serialize(&account()).unwrap(),
    );
    let mut group = c.benchmark_group("read_last_field");
    group.bench_function("borsh", |b| {
        b.iter(|| {
            Account::try_from_slice(black_box(&borsh))
                .unwrap()
                .members
                .len()
        })
    });
    group.bench_function("wincode_borrowed", |b| {
        b.iter(|| {
            wincode::deserialize::<AccountRef>(black_box(&wincode))
                .unwrap()
                .members
                .len()
        })
    });
    group.bench_function("zeropod_view", |b| {
        b.iter(|| Account::view(black_box(&borsh)).unwrap().members().len())
    });
    group.finish();
}

/// Changing a fixed field and leaving the bytes encoding the new value.
fn update_fixed_field(c: &mut Criterion) {
    let (borsh, wincode) = (
        borsh::to_vec(&account()).unwrap(),
        wincode::serialize(&account()).unwrap(),
    );
    let mut group = c.benchmark_group("update_fixed_field");
    group.bench_function("borsh", |b| {
        let mut bytes = borsh.clone();
        b.iter(|| {
            let mut value = Account::try_from_slice(&bytes).unwrap();
            value.amount += 1;
            borsh::to_writer(&mut bytes[..], &value).unwrap();
        })
    });
    group.bench_function("wincode", |b| {
        let mut bytes = wincode.clone();
        b.iter(|| {
            let mut value = wincode::deserialize::<Account>(&bytes).unwrap();
            value.amount += 1;
            wincode::serialize_into(&mut bytes[..], &value).unwrap();
        })
    });
    group.bench_function("zeropod", |b| {
        let mut bytes = borsh.clone();
        b.iter(|| {
            let view = Account::view_mut(&mut bytes).unwrap();
            view.set_amount(view.amount() + 1).unwrap();
        })
    });
    group.finish();
}

/// Changing a string's length: the fields after it move.
fn update_variable_field(c: &mut Criterion) {
    let names = ["treasury", "operations fund"];
    let room = |bytes: &[u8]| [bytes, &[0; 64]].concat();
    let (borsh, wincode) = (
        room(&borsh::to_vec(&account()).unwrap()),
        room(&wincode::serialize(&account()).unwrap()),
    );
    let mut group = c.benchmark_group("update_variable_field");
    group.bench_function("borsh", |b| {
        let mut bytes = borsh.clone();
        let mut turn = 0;
        b.iter(|| {
            let mut value = Account::deserialize(&mut &bytes[..]).unwrap();
            turn ^= 1;
            value.name = names[turn].into();
            borsh::to_writer(&mut bytes[..], &value).unwrap();
        })
    });
    group.bench_function("wincode", |b| {
        let mut bytes = wincode.clone();
        let mut turn = 0;
        b.iter(|| {
            let mut value = wincode::deserialize::<Account>(&bytes).unwrap();
            turn ^= 1;
            value.name = names[turn].into();
            wincode::serialize_into(&mut bytes[..], &value).unwrap();
        })
    });
    group.bench_function("zeropod", |b| {
        let mut bytes = borsh.clone();
        let mut turn = 0;
        b.iter(|| {
            turn ^= 1;
            Account::view_mut(&mut bytes)
                .unwrap()
                .set_name(names[turn])
                .unwrap();
        })
    });
    group.finish();
}

criterion_group!(
    compare,
    encode,
    decode,
    read_last_field,
    update_fixed_field,
    update_variable_field
);
criterion_main!(compare);
