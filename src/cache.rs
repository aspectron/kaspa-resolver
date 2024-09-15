use axum::{
    body::Body,
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
};

#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct NoCacheHtml<T>(pub T);

impl<T> IntoResponse for NoCacheHtml<T>
where
    T: Into<Body>,
{
    fn into_response(self) -> Response {
        (
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(mime::TEXT_HTML_UTF_8.as_ref()),
                ),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static(
                        "no-cache, no-store, must-revalidate, proxy-revalidate, max-age=0",
                    ),
                ),
            ],
            self.0.into(),
        )
            .into_response()
    }
}

impl<T> From<T> for NoCacheHtml<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}
