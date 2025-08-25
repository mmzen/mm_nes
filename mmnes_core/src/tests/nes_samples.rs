use crate::nes_samples::NesSamples;

#[test]
fn test_append_samples_to_empty_collection() {
    let mut empty_samples = NesSamples::default();
    let test_samples = vec![0.1, 0.2, 0.3, 0.4];
    let samples_to_append = NesSamples::new(test_samples.clone());

    empty_samples.append(samples_to_append);

    assert_eq!(empty_samples.samples(), test_samples);
}

#[test]
fn test_append_samples_to_non_empty_collection() {
    let initial_samples = vec![0.5, 0.6];
    let mut existing_samples = NesSamples::new(initial_samples.clone());
    let test_samples = vec![0.1, 0.2, 0.3, 0.4];
    let samples_to_append = NesSamples::new(test_samples.clone());

    existing_samples.append(samples_to_append);

    let expected_samples = vec![0.5, 0.6, 0.1, 0.2, 0.3, 0.4];
    assert_eq!(existing_samples.samples(), expected_samples);
}

