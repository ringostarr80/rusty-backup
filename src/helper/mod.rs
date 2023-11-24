use std::collections::VecDeque;
use std::ops::{Add, Sub};
use std::time::{Duration, Instant};

pub struct ProgressStats {
    pub total_length: Option<usize>,
    pub progressed_size: usize,
    finished: bool,
    start_time: Instant,
    time_series: VecDeque<(Instant, usize)>,
}

impl ProgressStats {
    pub fn new() -> ProgressStats {
        ProgressStats {
            total_length: None,
            progressed_size: 0,
            finished: false,
            start_time: std::time::Instant::now(),
            time_series: VecDeque::new(),
        }
    }

    pub fn add_progressed_size(&mut self, size: usize) {
        self.progressed_size += size;
        self.time_series.push_back((Instant::now(), size));
        loop {
            match self.time_series.get(0) {
                Some(tuple) => {
                    if Instant::now().sub(tuple.0) > Duration::from_secs(10) {
                        self.time_series.pop_front();
                    } else {
                        break;
                    }
                }
                None => {
                    break;
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn set_progressed_size(&mut self, size: usize) {
        let added_size = size - self.progressed_size;
        self.progressed_size = size;
        self.time_series.push_back((Instant::now(), added_size));
        loop {
            match self.time_series.get(0) {
                Some(tuple) => {
                    if Instant::now().sub(tuple.0) > Duration::from_secs(10) {
                        self.time_series.pop_front();
                    } else {
                        break;
                    }
                }
                None => {
                    break;
                }
            }
        }
    }

    fn get_formatted_duration(duration: Duration) -> String {
        let total_seconds = duration.as_secs_f64();
        let mut hours = 0;
        let mut minutes = 0;
        let mut seconds = 0;

        if total_seconds > 0.0 {
            seconds = total_seconds as i64;
            if seconds >= 60 {
                minutes = seconds / 60;
                seconds = seconds % 60;
            }
            if minutes >= 60 {
                hours = minutes / 60;
                minutes = minutes % 60;
            }
        }

        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }

    pub fn get_formatted_ete(&self) -> Option<String> {
        match self.get_ete() {
            Some(ete) => Some(Self::get_formatted_duration(ete)),
            None => None,
        }
    }

    pub fn get_formatted_runtime(&self) -> String {
        Self::get_formatted_duration(self.get_runtime())
    }

    /// Gets the average download speed in Bytes per second.
    pub fn get_average_speed(&self) -> usize {
        let total_seconds = self.get_runtime().as_secs_f64();
        (self.progressed_size as f64 / total_seconds) as usize
    }

    /// Gets the average download speed in Bytes per second.
    pub fn get_average_speed_for_last_second(&self) -> usize {
        self.get_average_speed_for_last_x_seconds(1)
    }

    /// Gets the average download speed in Bytes per second.
    pub fn get_average_speed_for_last_10_seconds(&self) -> usize {
        self.get_average_speed_for_last_x_seconds(10)
    }

    fn get_average_speed_for_last_x_seconds(&self, seconds: u64) -> usize {
        let now = Instant::now();
        let mut bytes_progressed = 0_usize;
        let mut first_found_progressed_time = None;
        for tuple in &self.time_series {
            if now.sub(tuple.0) > Duration::from_secs(seconds) {
                continue;
            }

            match first_found_progressed_time {
                Some(_) => {}
                None => {
                    first_found_progressed_time = Some(tuple.0);
                }
            }
            bytes_progressed += tuple.1;
        }

        let last_seconds = match first_found_progressed_time {
            Some(time) => {
                let mut secs = now.sub(time).as_secs();
                if secs == 0 {
                    secs = 1;
                }
                secs
            }
            None => seconds,
        };

        bytes_progressed / last_seconds as usize
    }

    pub fn get_progress_in_percentage(&self) -> Option<f64> {
        match self.total_length {
            Some(total_length) => Some(self.progressed_size as f64 * 100.0 / total_length as f64),
            None => None,
        }
    }

    pub fn _get_start_time(&self) -> Instant {
        self.start_time
    }

    pub fn get_runtime(&self) -> Duration {
        Instant::now().sub(self.start_time)
    }

    pub fn _get_eta(&self) -> Option<Instant> {
        match self.get_ete() {
            Some(ete) => Some(Instant::now().add(ete)),
            None => None,
        }
    }

    /// Gets the "Estimated Time Enroute".
    pub fn get_ete(&self) -> Option<Duration> {
        match self.get_total_duration() {
            Some(total_duration) => {
                if total_duration > self.get_runtime() {
                    Some(total_duration.sub(self.get_runtime()))
                } else {
                    Some(Duration::from_secs(0))
                }
            }
            None => None,
        }
    }

    pub fn get_total_duration(&self) -> Option<Duration> {
        match self.get_progress_in_percentage() {
            Some(percentage) => {
                if percentage > 0.0 {
                    let runtime = self.get_runtime();
                    Some(runtime.mul_f64(100.0 / percentage))
                } else {
                    None
                }
            }
            None => None,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn set_finished(&mut self) {
        self.finished = true;
    }
}
