use super::frames::Frames;
use crate::diagnostics;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread;

// Windows 桌面版没有可用的 ffmpeg 音频输出设备，解码后的 PCM 由 cpal 播放
pub(super) fn start(mut pcm: impl Read + Send + 'static, _frames: Arc<Frames>) {
    thread::spawn(move || {
        if let Err(error) = run(&mut pcm) {
            diagnostics::write(format_args!("[音频] 输出失败: {error}"));
        }
    });
}

fn run(pcm: &mut impl Read) -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or(crate::i18n::tr("没有可用的音频输出设备"))?;
    let supported = device
        .supported_output_configs()?
        .find(|config| {
            config.channels() >= 2
                && config.min_sample_rate().0 <= 44100
                && config.max_sample_rate().0 >= 44100
                && config.sample_format() == cpal::SampleFormat::F32
        })
        .ok_or(crate::i18n::tr("没有 44100Hz 双声道输出配置"))?;
    let config = supported
        .with_sample_rate(cpal::SampleRate(44100))
        .config();
    let channels = config.channels as usize;
    let queue = Arc::new(Mutex::new(VecDeque::<f32>::new()));

    let callback_queue = queue.clone();
    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _| {
            let mut queue = callback_queue.lock().unwrap();
            for frame in data.chunks_mut(channels) {
                let sample = queue.pop_front().unwrap_or(0.0);
                for value in frame {
                    *value = sample;
                }
            }
        },
        |error| diagnostics::write(format_args!("[音频] 输出流错误: {error}")),
        None,
    )?;
    stream.play()?;

    let mut bytes = [0u8; 8192];
    loop {
        let len = pcm.read(&mut bytes)?;
        if len == 0 {
            break;
        }
        {
            let mut buffer = queue.lock().unwrap();
            for pair in bytes[..len].chunks_exact(2) {
                buffer.push_back(i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32768.0);
            }
        }
        // 队列超过约两秒音频时等待播放消耗，避免无限堆积
        while queue.lock().unwrap().len() > 44100 * 2 * 2 {
            thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    Ok(())
}
