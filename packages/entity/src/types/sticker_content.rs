use serde::{Deserialize, Serialize};

/// Content for m.sticker events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerContent {
    /// A textual representation or associated description of the sticker image
    pub body: String,
    
    /// Metadata about the image referred to in url including a thumbnail representation
    pub info: StickerImageInfo,
    
    /// The URL to the sticker image. This must be a valid mxc:// URI
    pub url: String,
}

/// Image information for sticker content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerImageInfo {
    /// The intended display height of the image in pixels
    pub h: Option<u32>,
    
    /// The intended display width of the image in pixels
    pub w: Option<u32>,
    
    /// The mimetype of the image, e.g. image/jpeg
    pub mimetype: Option<String>,
    
    /// Size of the image in bytes
    pub size: Option<u32>,
    
    /// The URL to a thumbnail of the image
    pub thumbnail_url: Option<String>,
    
    /// Metadata about the thumbnail image
    pub thumbnail_info: Option<ThumbnailInfo>,
}

/// Thumbnail information for sticker images
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailInfo {
    /// The intended display height of the thumbnail in pixels
    pub h: Option<u32>,
    
    /// The intended display width of the thumbnail in pixels
    pub w: Option<u32>,
    
    /// The mimetype of the thumbnail, e.g. image/jpeg
    pub mimetype: Option<String>,
    
    /// Size of the thumbnail in bytes
    pub size: Option<u32>,
}

impl StickerContent {
    /// Create a new sticker content
    pub fn new(body: String, url: String, info: StickerImageInfo) -> Self {
        Self { body, url, info }
    }
    
    /// Validate sticker content according to Matrix specification
    pub fn validate(&self) -> Result<(), String> {
        // Validate URL format
        if !self.url.starts_with("mxc://") {
            return Err("Sticker URL must be a valid mxc:// URI".to_string());
        }
        
        // Validate image dimensions (recommended 512x512 or smaller)
        if let (Some(w), Some(h)) = (self.info.w, self.info.h) {
            if w > 512 || h > 512 {
                return Err("Sticker dimensions should be 512x512 pixels or smaller".to_string());
            }
        }
        
        // Validate body is not empty
        if self.body.trim().is_empty() {
            return Err("Sticker body cannot be empty".to_string());
        }
        
        Ok(())
    }
}