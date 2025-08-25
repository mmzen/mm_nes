

pub struct NesSamples {
    samples: Vec<f32>,
}

impl NesSamples {
    pub fn new(samples: Vec<f32>) -> Self {
        NesSamples {
            samples,
        }
    }

    pub fn append(&mut self, samples: NesSamples) {
        self.samples.extend(samples.samples);
    }

    pub fn samples(&self) -> &[f32] {
        &self.samples
    }
}

impl Default for NesSamples {
    fn default() -> Self {
        NesSamples {
            samples: Vec::new(),
        }
    }
}

impl Iterator for NesSamples {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        self.samples.pop()
    }
}