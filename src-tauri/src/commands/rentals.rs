use crate::rentals::{self, GithubRepoSelection, RentalDto, RentalStatus, RentalsState};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn start_rental(
    app: AppHandle,
    state: State<'_, RentalsState>,
    account_id: String,
    machine_profile: String,
    project_name: String,
    vm_username: String,
    vm_password: String,
    github_repo: Option<GithubRepoSelection>,
) -> Result<RentalDto, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let record = rentals::new_record(
        id.clone(),
        account_id.clone(),
        machine_profile.clone(),
        project_name,
        vm_username.clone(),
        github_repo,
    );
    let dto = record.to_dto();

    {
        let mut map = state.0.lock().await;
        map.insert(id.clone(), record);
    }

    let app_for_task = app.clone();
    tauri::async_runtime::spawn(async move {
        rentals::provision(
            app_for_task,
            id,
            account_id,
            machine_profile,
            vm_username,
            vm_password,
        )
        .await;
    });

    Ok(dto)
}

#[tauri::command]
pub async fn get_rental(state: State<'_, RentalsState>, id: String) -> Result<RentalDto, String> {
    let map = state.0.lock().await;
    map.get(&id)
        .map(|r| r.to_dto())
        .ok_or_else(|| format!("No rental with id {id}"))
}

#[tauri::command]
pub async fn get_provisioning_log(app: AppHandle, id: String) -> Result<Vec<String>, String> {
    Ok(rentals::get_provisioning_log(&app, &id).await)
}

#[tauri::command]
pub async fn stop_rental(
    app: AppHandle,
    state: State<'_, RentalsState>,
    id: String,
) -> Result<RentalDto, String> {
    let dto_after_mark = {
        let mut map = state.0.lock().await;
        let record = map
            .get_mut(&id)
            .ok_or_else(|| format!("No rental with id {id}"))?;

        if matches!(record.status, RentalStatus::Released) {
            return Ok(record.to_dto());
        }

        record.status = RentalStatus::Stopping;
        record.to_dto()
    };

    let app_for_task = app.clone();
    let id_for_task = id.clone();
    tauri::async_runtime::spawn(async move {
        rentals::stop(app_for_task, id_for_task).await;
    });

    Ok(dto_after_mark)
}
