use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn cannot_lookup(c: &mut Criterion) {
	c.bench_function("metadata_implicit_cannot_lookup", |b| {
		b.iter(|| {
			let result = orbis_metadata_implicit_nohash::resolve();
			assert_eq!(
				black_box(result),
				Err(sp_runtime::transaction_validity::UnknownTransaction::CannotLookup.into()),
			);
		})
	});
}

criterion_group!(benches, cannot_lookup);
criterion_main!(benches);
