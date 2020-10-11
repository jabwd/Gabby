use serde::{Serialize, Deserialize};

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

#[derive(Serialize, Debug)]
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
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VoiceRequest {
    pub input: VoiceInput,
    pub voice: Voice,
    pub audio_config: AudioConfig,
}
