use crate::domain::media::ContainerFormat;
use crate::domain::metadata::EditableMetadata;
use crate::domain::subtitle::{SubtitleDeliveryMode, SubtitleResolutionPlan};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfmpegMuxPlan {
    pub args: Vec<String>,
    pub output_file: PathBuf,
    pub external_sidecar_subs: Vec<PathBuf>,
}

/// Builds lossless `ffmpeg n9.0.2` remuxing commands for `.mkv` and `.mp4` (with sidecar `.srt` support).
pub struct FfmpegCommandBuilder;

impl FfmpegCommandBuilder {
    #[must_use]
    pub fn build_video_mux_plan(
        video_input: &Path,
        audio_input: Option<&Path>,
        downloaded_sub_files: &[(usize, PathBuf)],
        cover_art_file: Option<&Path>,
        metadata: &EditableMetadata,
        subtitle_plan: &SubtitleResolutionPlan,
        container: ContainerFormat,
        output_dir: &Path,
        file_stem: &str,
    ) -> FfmpegMuxPlan {
        let mut args = vec!["-hide_banner".to_string(), "-y".to_string()];

        // Input 0: Video
        args.push("-i".to_string());
        args.push(video_input.to_string_lossy().into_owned());

        // Input 1 (optional): Audio
        let mut next_input_idx = 1usize;
        let audio_idx = if let Some(aud) = audio_input {
            args.push("-i".to_string());
            args.push(aud.to_string_lossy().into_owned());
            let idx = next_input_idx;
            next_input_idx += 1;
            Some(idx)
        } else {
            None
        };

        // Subtitle inputs (for MKV internal mux OR MP4 soft-sub mov_text mux)
        let embed_subs_in_container = matches!(
            subtitle_plan.delivery_mode,
            SubtitleDeliveryMode::MkvInternalMux
                | SubtitleDeliveryMode::Mp4ExternalSidecarAndMovText
        );

        let mut sub_input_indices = Vec::new();
        if embed_subs_in_container {
            for (track_pos, sub_path) in downloaded_sub_files {
                args.push("-i".to_string());
                args.push(sub_path.to_string_lossy().into_owned());
                sub_input_indices.push((*track_pos, next_input_idx));
                next_input_idx += 1;
            }
        }

        // Map video & audio streams
        args.extend(["-map".to_string(), "0:v:0".to_string()]);
        if let Some(a_idx) = audio_idx {
            args.extend(["-map".to_string(), format!("{a_idx}:a:0")]);
        } else {
            args.extend(["-map".to_string(), "0:a:0?".to_string()]);
        }

        // Map subtitle streams
        for (_, s_in_idx) in &sub_input_indices {
            args.extend(["-map".to_string(), format!("{s_in_idx}:s:0")]);
        }

        // Stream copy video & audio (zero re-encoding!)
        args.extend([
            "-c:v".to_string(),
            "copy".to_string(),
            "-c:a".to_string(),
            "copy".to_string(),
        ]);

        // Subtitle codec & ISO-639-2 language metadata
        if !sub_input_indices.is_empty() {
            let sub_codec = match container {
                ContainerFormat::Mkv => "srt",
                ContainerFormat::Mp4 => "mov_text",
                _ => "copy",
            };
            args.extend(["-c:s".to_string(), sub_codec.to_string()]);

            for (out_s_idx, (track_pos, _)) in sub_input_indices.iter().enumerate() {
                if let Some(resolved_track) = subtitle_plan.tracks.get(*track_pos) {
                    args.extend([
                        format!("-metadata:s:s:{out_s_idx}"),
                        format!("language={}", resolved_track.language.iso_639_2_mkv()),
                        format!("-metadata:s:s:{out_s_idx}"),
                        format!(
                            "title={} [{}]",
                            resolved_track.language.display_name(),
                            resolved_track.origin.badge()
                        ),
                    ]);
                    if resolved_track.default_track {
                        args.extend([format!("-disposition:s:{out_s_idx}"), "default".to_string()]);
                    }
                }
            }
        }

        // Container-level metadata tags
        args.extend(["-metadata".to_string(), format!("title={}", metadata.title)]);
        if let Some(artist) = &metadata.artist_or_creator {
            args.extend(["-metadata".to_string(), format!("artist={artist}")]);
        }
        if let Some(year) = &metadata.release_year {
            args.extend(["-metadata".to_string(), format!("date={year}")]);
        }
        if let Some(comment) = &metadata.comment_or_description {
            args.extend(["-metadata".to_string(), format!("comment={comment}")]);
        }

        // MKV Cover Art Attachment
        if container == ContainerFormat::Mkv && metadata.embed_cover_art {
            if let Some(cover) = cover_art_file {
                args.extend([
                    "-attach".to_string(),
                    cover.to_string_lossy().into_owned(),
                    "-metadata:s:t:0".to_string(),
                    "mimetype=image/jpeg".to_string(),
                    "-metadata:s:t:0".to_string(),
                    "filename=cover.jpg".to_string(),
                ]);
            }
        }

        let output_file = output_dir.join(format!("{file_stem}.{}", container.extension()));
        args.push(output_file.to_string_lossy().into_owned());

        // Compute external sidecar .srt destinations when container is MP4 (or ExternalSidecarSrtOnly)
        let mut external_sidecar_subs = Vec::new();
        if subtitle_plan.delivery_mode.writes_external_sidecar() {
            for (track_pos, _) in downloaded_sub_files {
                if let Some(resolved_track) = subtitle_plan.tracks.get(*track_pos) {
                    let lang_code = resolved_track.language.iso_639_1();
                    let sidecar_path = output_dir.join(format!("{file_stem}.{lang_code}.srt"));
                    external_sidecar_subs.push(sidecar_path);
                }
            }
        }

        FfmpegMuxPlan {
            args,
            output_file,
            external_sidecar_subs,
        }
    }
}
