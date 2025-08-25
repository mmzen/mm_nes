use crate::sound_playback::SoundPlayback;
use crate::sound_playback_passive::SoundPlaybackPassive;

#[test]
fn test_push_sample_adds_single_sample_to_buffer() {
    let mut sound_playback = SoundPlaybackPassive::new();
    let sample = 0.5f32;

    sound_playback.push_sample(sample);

    let samples = sound_playback.samples();
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0], sample);
}

#[test]
fn test_push_sample_adds_multiple_samples_up_to_buffer_size_limit() {
    let mut sound_playback = SoundPlaybackPassive::new();

    for i in 0..1024 {
        sound_playback.push_sample(i as f32);
    }

    sound_playback.push_sample(1024.0);

    let samples = sound_playback.samples();
    assert_eq!(samples.len(), 1024);

    for i in 0..1024 {
        assert_eq!(samples[i], i as f32);
    }
}

#[test]
fn test_samples_clears_buffer_after_returning_copy() {
    let mut sound_playback = SoundPlaybackPassive::new();

    sound_playback.push_sample(1.0);
    sound_playback.push_sample(2.0);
    sound_playback.push_sample(3.0);

    let first_batch = sound_playback.samples();
    assert_eq!(first_batch.len(), 3);
    assert_eq!(first_batch, vec![1.0, 2.0, 3.0]);

    let second_batch = sound_playback.samples();
    assert_eq!(second_batch.len(), 0);

    sound_playback.push_sample(4.0);
    let third_batch = sound_playback.samples();
    assert_eq!(third_batch.len(), 1);
    assert_eq!(third_batch[0], 4.0);
}

#[test]
fn test_samples_returns_all_buffered_samples() {
    let mut sound_playback = SoundPlaybackPassive::new();

    let test_samples = vec![0.1, 0.2, 0.3, 0.4, 0.5];

    for sample in &test_samples {
        sound_playback.push_sample(*sample);
    }

    let returned_samples = sound_playback.samples();

    assert_eq!(returned_samples.len(), test_samples.len());
    assert_eq!(returned_samples, test_samples);
}