use crate::{
    auth::{AuthHttp, application_failure, private_headers},
    organizations::private_json,
    projects::{authenticated,bad_input,html_page},
};
use axum::{
    Extension,Json,Router,
    extract::{DefaultBodyLimit,Path,Query,rejection::{JsonRejection,QueryRejection}},
    http::{HeaderMap,HeaderValue,StatusCode,header},
    response::{IntoResponse,Response},
    routing::{get,post},
};
use canteiro_contracts::{estimating::*,reliability::ReliabilityScope};
use serde::Deserialize;

pub fn routes<S:Clone+Send+Sync+'static>()->Router<S>{
    Router::new()
        .route("/api/estimating/context",get(context))
        .route("/api/estimating/catalog",get(catalog))
        .route("/api/estimating/catalog/publish",post(catalog_publish))
        .route("/api/estimating/composition/calculate",post(composition_calculate))
        .route("/api/estimating/composition/command",post(composition_write))
        .route("/api/estimating/budget/lease",post(budget_lease))
        .route("/api/estimating/budget/command",post(budget_write))
        .route("/api/estimating/budget/import/preview",post(import_preview))
        .route("/api/estimating/budget/{scope}/history",get(budget_history))
        .route("/api/estimating/budget/{scope}/export.csv",get(budget_export))
        .route("/orcamentos",get(page))
        .layer(DefaultBodyLimit::max(9_000_000))
}
async fn context(auth:Option<Extension<AuthHttp>>,headers:HeaderMap)->Response{
    let (auth,token)=match authenticated(auth,&headers,false){
        Ok(v)=>v,Err(e)=>return *e,
    };
    match auth.service().estimating_context(&token).await{
        Ok(result)=>private_json(result),Err(e)=>application_failure(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogQuery{
    organization_id:String,context_version:i64,scope_id:String,
    catalog_id:String,edition:u32,
}
async fn catalog(
    auth:Option<Extension<AuthHttp>>,headers:HeaderMap,
    query:Result<Query<CatalogQuery>,QueryRejection>
)->Response{
    let (auth,token)=match authenticated(auth,&headers,false){
        Ok(v)=>v,Err(e)=>return *e,
    };
    let Ok(Query(query))=query else{return bad_input()};
    let context=ReliabilityScope{
        organization_id:query.organization_id,
        context_version:query.context_version,
    };
    match auth.service().estimating_catalog(
        &token,&context,&query.scope_id,&query.catalog_id,query.edition
    ).await{
        Ok(result)=>private_json(result),Err(e)=>application_failure(e),
    }
}
async fn catalog_publish(
    auth:Option<Extension<AuthHttp>>,headers:HeaderMap,
    input:Result<Json<CatalogPublishCommand>,JsonRejection>,
)->Response{
    let (auth,token)=match authenticated(auth,&headers,true){
        Ok(v)=>v,Err(e)=>return *e,
    };
    let Ok(Json(input))=input else{return bad_input()};
    match auth.service().estimating_catalog_publish(&token,input).await{
        Ok(result)=>private_json(result),Err(e)=>application_failure(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompositionCalculateCommand{
    context:ReliabilityScope,scope_id:String,composition:Composition,
}
async fn composition_calculate(
    auth:Option<Extension<AuthHttp>>,headers:HeaderMap,
    input:Result<Json<CompositionCalculateCommand>,JsonRejection>,
)->Response{
    let (auth,token)=match authenticated(auth,&headers,true){
        Ok(v)=>v,Err(e)=>return *e,
    };
    let Ok(Json(input))=input else{return bad_input()};
    match auth.service().estimating_composition_calculate(
        &token,&input.context,&input.scope_id,input.composition
    ).await{
        Ok(result)=>private_json(result),Err(e)=>application_failure(e),
    }
}
async fn composition_write(
    auth:Option<Extension<AuthHttp>>,headers:HeaderMap,
    input:Result<Json<CompositionWriteCommand>,JsonRejection>,
)->Response{
    let (auth,token)=match authenticated(auth,&headers,true){
        Ok(v)=>v,Err(e)=>return *e,
    };
    let Ok(Json(input))=input else{return bad_input()};
    match auth.service().estimating_composition_write(&token,input).await{
        Ok(result)=>private_json(result),Err(e)=>application_failure(e),
    }
}
async fn budget_lease(
    auth:Option<Extension<AuthHttp>>,headers:HeaderMap,
    input:Result<Json<BudgetLeaseCommand>,JsonRejection>,
)->Response{
    let (auth,token)=match authenticated(auth,&headers,true){
        Ok(v)=>v,Err(e)=>return *e,
    };
    let Ok(Json(input))=input else{return bad_input()};
    match auth.service().estimating_budget_lease(&token,input).await{
        Ok(result)=>private_json(result),Err(e)=>application_failure(e),
    }
}
async fn budget_write(
    auth:Option<Extension<AuthHttp>>,headers:HeaderMap,
    input:Result<Json<BudgetCommand>,JsonRejection>,
)->Response{
    let (auth,token)=match authenticated(auth,&headers,true){
        Ok(v)=>v,Err(e)=>return *e,
    };
    let Ok(Json(input))=input else{return bad_input()};
    match auth.service().estimating_budget_execute(&token,input).await{
        Ok(result)=>private_json(result),Err(e)=>application_failure(e),
    }
}
async fn import_preview(
    auth:Option<Extension<AuthHttp>>,headers:HeaderMap,
    input:Result<Json<BudgetImportPreviewCommand>,JsonRejection>,
)->Response{
    let (auth,token)=match authenticated(auth,&headers,true){
        Ok(v)=>v,Err(e)=>return *e,
    };
    let Ok(Json(input))=input else{return bad_input()};
    match auth.service().estimating_budget_import_preview(&token,input).await{
        Ok(result)=>private_json(result),Err(e)=>application_failure(e),
    }
}
async fn budget_history(
    auth:Option<Extension<AuthHttp>>,headers:HeaderMap,
    Path(scope):Path<String>,
    query:Result<Query<ReliabilityScope>,QueryRejection>,
)->Response{
    let (auth,token)=match authenticated(auth,&headers,false){
        Ok(v)=>v,Err(e)=>return *e,
    };
    let Ok(Query(context))=query else{return bad_input()};
    match auth.service().estimating_budget_history(&token,&context,&scope).await{
        Ok(result)=>private_json(result),Err(e)=>application_failure(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportQuery{
    organization_id:String,context_version:i64,locale:NumberLocale,
}
async fn budget_export(
    auth:Option<Extension<AuthHttp>>,headers:HeaderMap,
    Path(scope):Path<String>,query:Result<Query<ExportQuery>,QueryRejection>,
)->Response{
    let (auth,token)=match authenticated(auth,&headers,false){
        Ok(v)=>v,Err(e)=>return *e,
    };
    let Ok(Query(query))=query else{return bad_input()};
    let context=ReliabilityScope{
        organization_id:query.organization_id,context_version:query.context_version
    };
    match auth.service().estimating_budget_export_csv(
        &token,&context,&scope,query.locale
    ).await{
        Ok(csv)=>{
            let mut response=csv.into_response();
            private_headers(&mut response);
            response.headers_mut().insert(header::CONTENT_TYPE,
                HeaderValue::from_static("text/csv; charset=utf-8"));
            response.headers_mut().insert(header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment; filename=\"canteiro-orcamento.csv\""));
            response
        },
        Err(e)=>application_failure(e),
    }
}
async fn page(auth:Option<Extension<AuthHttp>>,headers:HeaderMap)->Response{
    let (auth,token)=match authenticated(auth,&headers,false){
        Ok(v)=>v,Err(e)=>return *e,
    };
    match auth.service().estimating_context(&token).await{
        Ok(ctx)=>html_page(canteiro_web::estimating::page(ctx),StatusCode::OK),
        Err(e)=>application_failure(e),
    }
}
