use serde::{Serialize, Deserialize};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VoiceListEntity {
    pub language_codes: Vec<String>,
    pub name: String,
    pub ssml_gender: String,
    pub natural_sample_rate_hertz: u64,
}

#[derive(Deserialize, Debug)]
pub struct VoiceListResponseEntity {
    pub voices: Vec<VoiceListEntity>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VoiceResponse {
    pub audio_content: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VoiceInput {
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    pub language_code: String,
    pub name: String,
    pub ssml_gender: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AudioConfig {
    pub audio_encoding: String,
    pub sample_rate_hertz: u32,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VoiceRequest {
    pub input: VoiceInput,
    pub voice: Voice,
    pub audio_config: AudioConfig,
}
