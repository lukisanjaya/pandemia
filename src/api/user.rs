//! Koleksi query yang digunakan untuk operasi pada rest API.
#![allow(missing_docs)]

use actix_web::{HttpRequest, HttpResponse};
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use validator::Validate;

use crate::crypto::{self, PublicKey, SecretKey, Signature};

use crate::{
    api,
    api::types::*,
    api::{
        error::{bad_request, param_error, unauthorized},
        parsed_query::*,
        ApiResult, Error as ApiError, HttpRequest as ApiHttpRequest,
    },
    auth,
    dao::{AuthDao, CityDao, VillageDao},
    error::{Error, ErrorCode},
    geolocator, models,
    prelude::*,
    types::AccountKind,
    util, ID,
};

#[derive(Deserialize, Validate)]
pub struct SetUserSetting {
    pub key: String,
    pub value: String,
}

#[derive(Deserialize)]
pub struct UpdatePassword {
    pub old_password: String,
    pub new_password: String,
    pub verif_new_password: String,
}

#[derive(Deserialize, Validate)]
pub struct UpdateUser {
    #[validate(length(min = 2, max = 64))]
    pub full_name: String,
    #[validate(length(min = 2, max = 50))]
    pub email: Option<String>,
    #[validate(length(min = 2, max = 15))]
    pub phone_num: String,

    /// akan deprecated, digantikan dengan loc_path
    // #[deprecated(note="digantikan dengan loc_path")]
    #[validate(length(min = 2, max = 100))]
    pub village: String,

    #[validate(length(min = 2, max = 100))]
    pub loc_path: Option<String>,
    pub latitude: f64,
    pub longitude: f64,

    #[validate(length(min = 2, max = 30))]
    pub area_code: String,
    pub is_medic: bool,
}

#[derive(Deserialize, Validate)]
pub struct UpdateAccesses {
    pub id: ID,
    pub accesses: Vec<String>,
}

use crate::models::AccessToken;

/// Holder untuk implementasi API endpoint publik untuk user.
pub struct PublicApi;

#[api_group("User", "public", base = "/user/v1")]
impl PublicApi {
    // /// Rest API endpoint untuk mendaftarkan akun baru.
    // /// Setelah register akun tidak langsung aktif, perlu melakukan
    // /// aktifasi menggunakan endpoint `/user/activate`.
    // #[api_endpoint(path = "/user/register", mutable, auth = "none")]
    // pub fn register_user(query: RegisterUser) -> ApiResult<String> {
    //     let conn = state.db();
    //     let schema = UserDao::new(&conn);

    //     schema
    //         .register_user(&query.full_name, &query.email, &query.phone_num)
    //         .map_err(From::from)
    //         .map(ApiResult::success)
    // }

    // /// Mengaktifkan user yang telah teregister.
    // /// Ini nantinya dijadikan link yang akan dikirimkan ke email pendaftar.
    // #[api_endpoint(path = "/user/activate", auth = "none", mutable)]
    // pub fn activate_user(query: ActivateUser) -> ApiResult<types::User> {
    //     let conn = state.db();
    //     let schema = UserDao::new(&conn);
    //     let user = schema.activate_registered_user(query.token)?;
    //     schema.set_password(user.id, &query.password)?;
    //     Ok(user.into())
    // }

    /// Mendapatkan informasi current user.
    #[api_endpoint(path = "/me/info", auth = "required")]
    pub fn me_info(state: &AppState, query: (), req: &ApiHttpRequest) -> ApiResult<User> {
        if current_user.is_deleted() {
            return unauthorized();
        }
        Ok(ApiResult::success(current_user.into()))
    }

    /// Update current user.
    #[api_endpoint(path = "/me/update", auth = "required", mutable)]
    pub fn update_current_user(query: UpdateUser) -> ApiResult<()> {
        query.validate()?;
        let conn = state.db();
        let dao = UserDao::new(&conn);

        let city = CityDao::new(&conn).get_by_area_code(&query.area_code)?;

        if city.is_none() {
            return param_error("Kode area tidak benar, mohon periksa kembali.");
        }
        let city = city.unwrap();

        let loc_info = match geolocator::ll_to_address(query.latitude, query.longitude, &conn) {
            Ok(loc_info) => Some(loc_info),
            Err(e) => {
                error!("Cannot get geo locator. {}", e);
                None
            }
        };

        // get village id
        let village = match VillageDao::new(&conn).get_by_name_str(&city.province, &city.name, &query.village)
        {
            Ok(a) => a,
            Err(_) => {
                return param_error(&format!(
                    "Tidak dapat menemukan data untuk desa {}",
                    query.village
                ))
            }
        };

        // check maks 2 satgas per daerah
        {
            use crate::schema::users::{self, dsl};
            let village = format!("village_id={}", village.id);
            if users::table
                .filter(dsl::meta.contains(&vec![":satgas:", &village]))
                .select(diesel::dsl::count(dsl::id))
                .first::<i64>(&conn)
                .map_err(Error::from)?
                > 1
            {
                return bad_request("Maksimal 2 satgas per desa");
            }
        }

        let mut meta: Vec<String> = Vec::new();

        // daftarkan sebagai satgas dan set metadata-nya
        meta.push(":satgas:".to_string());
        meta.push(format!("village={}", query.village));
        meta.push(format!("village_id={}", village.id));
        meta.push(format!("district_id={}", village.district_id));
        meta.push(format!("district={}", village.district_name));
        meta.push(format!("area_code={}", city.area_code));
        meta.push(format!("city_name={}", city.name));
        meta.push(format!("city_id={}", city.id));
        meta.push(format!("province_name={}", city.province));
        meta.push(format!("address_by_area_code={}/{}", city.province, city.name));
        meta.push("access.data".to_string());
        meta.push("access.data_person".to_string());
        if query.is_medic {
            meta.push(":medic:".to_string());
            meta.push("access.village_data".to_string());
        }

        if let Some(loc_info) = loc_info {
            meta.push(format!(
                "address={}/{}/{}/{}/{}/{}",
                loc_info.country_code,
                loc_info.province,
                loc_info.city.unwrap_or("?".to_string()),
                loc_info.district.unwrap_or("?".to_string()),
                loc_info.subdistrict.unwrap_or("?".to_string()),
                loc_info.label
            ));
        }

        dao.update_user_info(
            current_user.id,
            &query.full_name,
            &query.email,
            &query.phone_num,
            query.latitude,
            query.longitude,
            meta.iter().map(|a| a.as_str()).collect::<Vec<&str>>(),
        )?;
        Ok(ApiResult::success(()))
    }

    /// Update password.
    #[api_endpoint(path = "/update_password", auth = "required", mutable, accessor=["user", "admin"])]
    pub fn update_password(query: UpdatePassword) -> ApiResult<()> {
        let conn = state.db();
        let dao = UserDao::new(&conn);

        if query.new_password.len() < 6 {
            param_error("New password too short, please use min 6 characters long")?;
        }

        if query.new_password != query.verif_new_password {
            param_error("Password verification didn't match")?;
        }

        let auth_dao = auth::AuthDao::new(&conn);

        let user_passhash = auth_dao.get_passhash(AccountKind::User, current_user.id)?;
        if !crypto::password_match(&query.old_password, &user_passhash) {
            warn!(
                "user `{}` try to update password using wrong password",
                &current_user.id
            );
            Err(ApiError::Unauthorized)?
        }

        dao.set_password(current_user.id, &query.new_password)?;

        Ok(ApiResult::success(()))
    }

    /// Mendapatkan data user berdasarkan ID.
    #[api_endpoint(path = "/detail", auth = "required", accessor = "admin")]
    pub fn user_detail(query: IdQuery) -> ApiResult<User> {
        let conn = state.db();
        let dao = UserDao::new(&conn);

        let is_super_admin = current_admin.id == 1;

        dao.get_by_id(query.id)
            .map(|a| {
                let mut a: User = a.into();
                if !is_super_admin {
                    a.meta = vec![];
                }
                ApiResult::success(a)
            })
            .map_err(From::from)
    }

    /// Mendapatkan data user berdasarkan ID.
    #[api_endpoint(path = "/satgas/detail", auth = "required", accessor = "admin")]
    pub fn satgas_detail(query: IdQuery) -> ApiResult<Satgas> {
        let conn = state.db();
        let dao = UserDao::new(&conn);

        dao.get_by_id(query.id)
            .map(|a| ApiResult::success(a.to_api_type(&conn)))
            .map_err(From::from)
    }

    /// Update accesses.
    #[api_endpoint(path = "/update_accesses", auth = "required", mutable, accessor = "admin")]
    pub fn update_accesses(query: UpdateAccesses) -> ApiResult<()> {
        use crate::schema::users::{self, dsl};
        let conn = state.db();

        if current_admin.id != 1 {
            return unauthorized();
        }

        let user = UserDao::new(&conn).get_by_id(query.id)?;

        let mut meta = user.meta.clone();

        meta = meta.into_iter().filter(|a| !a.starts_with("access.")).collect();

        for acc in query.accesses {
            meta.push(format!("access.{}", acc));
        }

        diesel::update(dsl::users.filter(dsl::id.eq(query.id)))
            .set(dsl::meta.eq(meta))
            .execute(&conn)
            .map_err(Error::from)?;

        Ok(ApiResult::success(()))
    }

    /// Delete satgas.
    #[api_endpoint(path = "/satgas/delete", auth = "required", mutable, accessor = "admin")]
    pub fn delete_satgas(query: IdQuery) -> ApiResult<()> {
        let conn = state.db();
        let dao = UserDao::new(&conn);
        let user = dao.get_by_id(query.id)?;
        if current_admin.id != 1 {
            if !user.is_satgas() {
                return unauthorized();
            }
            if current_admin.get_city_id() != user.get_city_id() {
                return unauthorized();
            }
        }

        dao.mark_deleted(user.id)?;

        // clear up user's access token
        AuthDao::new(&conn).clear_access_token_by_user_id(user.id)?;

        Ok(ApiResult::success(()))
    }

    /// Delete satgas.
    #[api_endpoint(path = "/satgas/block", auth = "required", mutable, accessor = "admin")]
    pub fn block_satgas(query: IdQuery) -> ApiResult<()> {
        let conn = state.db();
        let dao = UserDao::new(&conn);
        let user = dao.get_by_id(query.id)?;
        if current_admin.id != 1 {
            if !user.is_satgas() {
                return unauthorized();
            }
            if current_admin.get_city_id() != user.get_city_id() {
                return unauthorized();
            }
        }

        dao.mark_blocked(user.id, true)?;

        Ok(ApiResult::success(()))
    }

    /// Delete satgas.
    #[api_endpoint(path = "/satgas/unblock", auth = "required", mutable, accessor = "admin")]
    pub fn unblock_satgas(query: IdQuery) -> ApiResult<()> {
        let conn = state.db();
        let dao = UserDao::new(&conn);
        let user = dao.get_by_id(query.id)?;
        if current_admin.id != 1 {
            if !user.is_satgas() {
                return unauthorized();
            }
            if current_admin.get_city_id() != user.get_city_id() {
                return unauthorized();
            }
        }

        dao.mark_blocked(user.id, false)?;

        Ok(ApiResult::success(()))
    }

    /// Register and connect current account to event push notif (FCM).
    /// Parameter `app_id` adalah app id dari client app.
    #[api_endpoint(path = "/me/connect/create", auth = "required", mutable)]
    pub fn connect_create(query: UserConnect) -> ApiResult<()> {
        query.validate()?;

        let conn = state.db();
        let dao = UserDao::new(&conn);

        dao.create_user_connect(
            current_user.id,
            &query.device_id,
            &query.provider_name,
            &query.app_id,
            &query.loc_name,
            &query.loc_name_full,
        )?;
        Ok(ApiResult::success(()))
    }

    /// Revoke or disconnect current account to event push notif (FCM).
    /// Parameter `app_id` adalah app id dari client app.
    #[api_endpoint(path = "/me/connect/remove", auth = "none", mutable)]
    pub fn connect_remove(query: UserConnect) -> ApiResult<()> {
        query.validate()?;

        let conn = state.db();
        let dao = UserDao::new(&conn);

        dao.remove_user_connect(&query.device_id, &query.provider_name, &query.app_id)?;
        Ok(ApiResult::success(()))
    }

    /// Update latest location
    #[api_endpoint(path = "/me/update_loc", auth = "required", mutable)]
    pub fn update_location(query: UpdateLocation) -> ApiResult<()> {
        let conn = state.db();
        let dao = UserDao::new(&conn);
        dao.update_user_location(
            &current_user,
            &query.device_id,
            &query.loc_name,
            &query.loc_name_full,
        )?;
        Ok(ApiResult::success(()))
    }

    /// Mendapatkan data akun.
    #[api_endpoint(path = "/user/info", accessor = "admin", auth = "required")]
    pub fn user_info(query: IdQuery) -> ApiResult<db::User> {
        let conn = state.db();
        let dao = UserDao::new(&conn);

        dao.get_by_id(query.id)
            .map(ApiResult::success)
            .map_err(From::from)
    }

    /// Update user settings.
    #[api_endpoint(path = "/update_setting", auth = "required", mutable)]
    pub fn update_setting(query: SetUserSetting) -> ApiResult<()> {
        use crate::schema::users::{self, dsl};
        let conn = state.db();

        current_user.set_setting(&query.key, &query.value, &conn)?;

        Ok(ApiResult::success(()))
    }

    /// Get user settings.
    #[api_endpoint(path = "/settings", auth = "required")]
    pub fn get_settings(query: ()) -> ApiResult<Vec<models::UserSetting>> {
        let conn = state.db();
        let user_settings = current_user.get_settings(&conn)?;

        Ok(ApiResult::success(user_settings))
    }

    /// Listing user
    #[api_endpoint(path = "/users", auth = "required", accessor = "admin")]
    pub fn list_user(query: QueryEntries) -> ApiResult<EntriesResult<User>> {
        let conn = state.db();
        let dao = UserDao::new(&conn);

        let entries = dao.get_users(query.offset, query.limit)?;

        let count = dao.count()?;
        Ok(ApiResult::success(EntriesResult {
            count,
            entries: entries.into_iter().map(|a| a.into()).collect(),
        }))
    }

    /// Mencari akun berdasarkan kata kunci.
    #[api_endpoint(path = "/search", auth = "required", accessor = "admin")]
    pub fn search_users(query: QueryEntries) -> ApiResult<EntriesResult<User>> {
        let conn = state.db();
        let dao = UserDao::new(&conn);

        if query.query.is_none() {
            return Self::list_user(&state, query, req);
        }

        let keyword = query.query.unwrap();

        let (entries, count) = dao.search(&keyword, query.offset, query.limit)?;

        Ok(ApiResult::success(EntriesResult {
            count,
            entries: entries.into_iter().map(|a| a.into()).collect(),
        }))
    }

    /// Mencari akun satgas berdasarkan kata kunci.
    #[api_endpoint(path = "/satgas/search", auth = "required", accessor = "admin")]
    pub fn satgas_search(query: QueryEntries) -> ApiResult<EntriesResult<Satgas>> {
        let conn = state.db();
        let dao = UserDao::new(&conn);

        let keyword = query.query.unwrap_or("".to_string());

        if !current_admin.has_access("satgas") && current_admin.id != 1 {
            return unauthorized();
        }

        let mut meta = vec![":satgas:"];
        let city = format!("city_id={}", current_admin.get_city_id().unwrap_or(0));
        if current_admin.id != 1 {
            meta.push(&city);
        }

        let excludes_meta = vec![":deleted:"];

        let parq = parse_query(&keyword);

        let village_name = parq.village_name.map(|a| util::title_case(&a));

        let sresult = dao.search_with_meta(
            &keyword,
            village_name.as_ref().map(|a| a.as_str()),
            &meta,
            &excludes_meta,
            query.offset,
            query.limit,
        )?;

        Ok(ApiResult::success(EntriesResult {
            count: sresult.count,
            entries: sresult
                .entries
                .into_iter()
                .map(|a| a.to_api_type(&conn))
                .collect(),
        }))
    }
}

use crate::models as db;

/// Holder untuk implementasi API endpoint privat.
pub struct PrivateApi;

#[api_group("User", "private", base = "/user/v1")]
impl PrivateApi {
    /// Listing user
    #[api_endpoint(path = "/users", auth = "none")]
    pub fn list_user(query: QueryEntries) -> ApiResult<EntriesResult<db::User>> {
        let conn = state.db();
        let dao = UserDao::new(&conn);

        let entries = dao.get_users(query.offset, query.limit)?;

        let count = dao.count()?;
        Ok(ApiResult::success(EntriesResult { count, entries }))
    }

    /// Mencari akun berdasarkan kata kunci.
    #[api_endpoint(path = "/search", auth = "none")]
    pub fn search_users(query: QueryEntries) -> ApiResult<EntriesResult<db::User>> {
        let conn = state.db();
        let dao = UserDao::new(&conn);

        if query.query.is_none() {
            return Self::list_user(&state, query, req);
        }

        let keyword = query.query.unwrap();

        let (entries, count) = dao.search(&keyword, query.offset, query.limit)?;

        Ok(ApiResult::success(EntriesResult { count, entries }))
    }

    /// Mendapatkan jumlah akun secara keseluruhan.
    #[api_endpoint(path = "/user/count")]
    pub fn user_count(state: &AppState, query: ()) -> ApiResult<i64> {
        let conn = state.db();
        let dao = UserDao::new(&conn);

        dao.count().map(ApiResult::success).map_err(From::from)
    }

    /// Mendapatkan data akun.
    #[api_endpoint(path = "/user/info", auth = "required")]
    pub fn user_info(query: IdQuery) -> ApiResult<db::User> {
        let conn = state.db();
        let dao = UserDao::new(&conn);

        dao.get_by_id(query.id)
            .map(ApiResult::success)
            .map_err(From::from)
    }
}
