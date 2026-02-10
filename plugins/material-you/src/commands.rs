use tauri::{command, AppHandle, Runtime};

use crate::models::MaterialYouResponse;
use crate::MaterialYouExt;

#[command]
pub(crate) async fn get_material_you_colours<R: Runtime>(
    app: AppHandle<R>,
) -> Result<MaterialYouResponse, String> {
    app.material_you()
        .get_material_you_colours()
        .map_err(|e| e.to_string())
}
