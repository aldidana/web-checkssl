use std::collections::HashMap;
use std::env;
use actix_web::{error, web, App, HttpServer, middleware, HttpResponse, Error, http::StatusCode, dev::ServiceResponse};
use actix_web::middleware::errhandlers::{ErrorHandlerResponse, ErrorHandlers};
use actix_http::{body::Body, Response};
use tera::Tera;
use checkssl::CheckSSL;

async fn index(
    tmpl: web::Data<tera::Tera>,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    let s = if let Some(domain) = query.get("domain") {
        match CheckSSL::from_domain(domain)  {
            Ok(cert) => {
                let mut ctx = tera::Context::new();
                ctx.insert("domain", &domain.to_owned());
                ctx.insert("cert", &cert);

                tmpl.render("ssl.html", &ctx)
                  .map_err(|_| error::ErrorInternalServerError("Template error"))?
            },
            Err(e) => {
                let mut ctx = tera::Context::new();
                ctx.insert("error", &e.to_string());

                tmpl.render("error.html", &ctx)
                  .map_err(|_| error::ErrorInternalServerError("Template error"))?
            }
        }
    } else {
        tmpl.render("index.html", &tera::Context::new())
          .map_err(|_| error::ErrorInternalServerError("Template error"))?
    };
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let mut port = &"8080".to_string();

    if args.len() > 1 {
        let is_numeric: Vec<bool> = args[1].chars().map(|c|c.is_numeric()).collect();
        if !is_numeric.contains(&false) {
            port = &args[1];
        }
    }

    println!("Server run on port {:}", port);

    HttpServer::new(|| {
        let tera = Tera::new(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/**/*")).unwrap();

        App::new()
          .data(tera)
          .wrap(middleware::Logger::default())
          .service(web::resource("/").route(web::get().to(index)))
          .service(web::scope("").wrap(error_handlers()))

    })
      .bind("127.0.0.1:".to_string() + port)?
      .run()
      .await
}

// Custom error handlers, to return HTML responses when an error occurs.
fn error_handlers() -> ErrorHandlers<Body> {
    ErrorHandlers::new().handler(StatusCode::NOT_FOUND, not_found)
}

// Error handler for a 404 Page not found error.
fn not_found<B>(res: ServiceResponse<B>) -> Result<ErrorHandlerResponse<B>, Error> {
    let response = get_error_response(&res, "Page not found");
    Ok(ErrorHandlerResponse::Response(
        res.into_response(response.into_body()),
    ))
}

// Generic error handler.
fn get_error_response<B>(res: &ServiceResponse<B>, error: &str) -> Response<Body> {
    let request = res.request();

    println!("{:?}", error);

    // Provide a fallback to a simple plain text response in case an error occurs during the
    // rendering of the error page.
    let fallback = |e: &str| {
        Response::build(res.status())
          .content_type("text/plain")
          .body(e.to_string())
    };

    let tera = request.app_data::<web::Data<Tera>>().map(|t| t.get_ref());
    match tera {
        Some(tera) => {
            let mut context = tera::Context::new();
            context.insert("error", error);
            context.insert("status_code", res.status().as_str());
            let body = tera.render("error.html", &context);

            match body {
                Ok(body) => Response::build(res.status())
                  .content_type("text/html")
                  .body(body),
                Err(_) => fallback(error),
            }
        }
        None => fallback(error),
    }
}