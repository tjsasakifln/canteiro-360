use crate::{
    auth::{AuthHttp, application_failure},
    organizations::private_json,
    projects::{authenticated, bad_input, field, form_context, form_headers, html_page},
};
use axum::{
    Extension,Form,Json,Router,
    extract::{DefaultBodyLimit,rejection::JsonRejection},
    http::{HeaderMap,StatusCode},
    response::{IntoResponse,Redirect,Response},
    routing::{get,post},
};
use canteiro_contracts::notifications::*;
use std::collections::BTreeMap;

pub fn routes<S:Clone+Send+Sync+'static>()->Router<S>{
    Router::new()
        .route("/api/notices/context",get(context))
        .route("/api/notices/preferences",post(preferences))
        .route("/pendencias",get(page))
        .route("/pendencias/preferencias",post(form))
        .layer(DefaultBodyLimit::max(8_192))
}
async fn context(auth:Option<Extension<AuthHttp>>,headers:HeaderMap)->Response{
    let (auth,token)=match authenticated(auth,&headers,false){
        Ok(value)=>value,Err(response)=>return *response,
    };
    match auth.service().notices_context(&token).await{
        Ok(value)=>private_json(value),
        Err(error)=>application_failure(error),
    }
}
async fn preferences(
    auth:Option<Extension<AuthHttp>>,headers:HeaderMap,
    input:Result<Json<NoticePreferencesCommand>,JsonRejection>,
)->Response{
    let (auth,token)=match authenticated(auth,&headers,true){
        Ok(value)=>value,Err(response)=>return *response,
    };
    let Ok(Json(input))=input else{return bad_input()};
    match auth.service().notices_set_preferences(&token,input).await{
        Ok(value)=>private_json(value),
        Err(error)=>application_failure(error),
    }
}
async fn page(auth:Option<Extension<AuthHttp>>,headers:HeaderMap)->Response{
    let (auth,token)=match authenticated(auth,&headers,false){
        Ok(value)=>value,Err(response)=>return *response,
    };
    match auth.service().notices_context(&token).await{
        Ok(value)=>html_page(canteiro_web::notifications::page(
            value,None,uuid::Uuid::new_v4().to_string()),StatusCode::OK),
        Err(error)=>application_failure(error),
    }
}
async fn form(
    auth:Option<Extension<AuthHttp>>,headers:HeaderMap,
    input:Result<Form<BTreeMap<String,String>>,axum::extract::rejection::FormRejection>,
)->Response{
    let Ok(Form(data))=input else{return bad_input()};
    let headers=match form_headers(headers,&data){Ok(v)=>v,Err(e)=>return *e};
    let (auth,token)=match authenticated(auth,&headers,true){
        Ok(value)=>value,Err(response)=>return *response,
    };
    let parsed=(||->Result<NoticePreferencesCommand,String>{
        let context=form_context(&data)?;
        let expected_version=if field(&data,"expected_version").is_empty(){None}
            else{Some(field(&data,"expected_version").parse()
                .map_err(|_|"Confira a versão atual das preferências.".to_owned())?)};
        let text=field(&data,"reminder_days").trim();
        let reminder_days=if text.is_empty(){vec![]}else{
            text.split(',').map(|part|part.trim().parse::<u16>()
                .map_err(|_|"Informe dias inteiros, separados por vírgula.".to_owned()))
                .collect::<Result<Vec<_>,_>>()?
        };
        Ok(NoticePreferencesCommand{context,expected_version,
            settings:NoticePreferences{
                mute_optional_email:field(&data,"mute_optional_email")=="true",
                group_by_day:field(&data,"group_by_day")=="true",
                reminder_days,
            }})
    })();
    let command=match parsed{
        Ok(v)=>v,
        Err(message)=>{
            return match auth.service().notices_context(&token).await{
                Ok(ctx)=>html_page(canteiro_web::notifications::page(ctx,
                    Some(message),uuid::Uuid::new_v4().to_string()),StatusCode::BAD_REQUEST),
                Err(e)=>application_failure(e),
            };
        }
    };
    match auth.service().notices_set_preferences(&token,command).await{
        Ok(_)=>Redirect::to("/pendencias").into_response(),
        Err(error)=>application_failure(error),
    }
}
