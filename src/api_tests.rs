#[cfg(test)]
mod tests {
    use actix_web::{
        App,
        dev::Service,
        http::StatusCode, test, web,
    };
    use diesel::RunQueryDsl;
    use futures::FutureExt;

    use lazy_static::lazy_static;

    use crate::addresses;
    use crate::data::models::{Address, AddressRecord};
    use crate::data::repo::addresses::create_or_update_addresses;
    use crate::db::{init_test_connection_pool, Pool};

    embed_migrations!("./migrations");

    lazy_static! {
        static ref POOL: Pool = {
            let pool = init_test_connection_pool();
            // Run migrations once for all tests
            embedded_migrations::run(&pool.get().unwrap())
                .expect("Error while running migrations");
            pool
        };
    }

    async fn run_test<F, R>(test: F) -> R
    where
        F: std::future::Future<Output = R>,
    {
        setup().await;
        // Make the fut UnwindSafe in order to catch unwind it.
        // That way we can run teardown even in case of failure.
        let test_fut = std::panic::AssertUnwindSafe(test).catch_unwind();
        let result = test_fut.await;
        teardown().await;
        assert!(result.is_ok());
        result.unwrap()
    }

    async fn setup() {
        use crate::data::schema::addresses;
        // Clear data from previous tests
        web::block(|| {
            diesel::delete(addresses::table)
                .execute(&POOL.get().unwrap())
        })
        .await
        .expect("Couldn't delete addresses table");
    }

    async fn teardown () {}

    #[actix_rt::test]
    async fn test_get_addresses_missing_query_param() {
        run_test(async {
            let mut app = test::init_service(
                App::new()
                    .data(POOL.clone())
                    .route("/addresses", web::get().to(addresses))
            )
            .await;

            let req = test::TestRequest::get()
                .uri("/addresses")
                .to_request();

            let resp = app.call(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        })
        .await
    }

    #[actix_rt::test]
    async fn test_get_no_addresses() {
        run_test(async {
            let mut app = test::init_service(
                App::new()
                    .data(POOL.clone())
                    .route("/addresses", web::get().to(addresses))
            )
            .await;

            let req = test::TestRequest::get()
                .uri("/addresses?postcode=2222AA")
                .to_request();

            let resp: Vec<Address> = test::read_response_json(&mut app, req).await;
            assert_eq!(resp.len(), 0);
        })
        .await
    }

    #[actix_rt::test]
    async fn test_get_addresses_postcode_only() {
        run_test(async {
            let mut app = test::init_service(
                App::new()
                    .data(POOL.clone())
                    .route("/addresses", web::get().to(addresses))
            )
            .await;

            create_test_set().await;

            let req = test::TestRequest::get()
                .uri("/addresses?postcode=2222AA")
                .to_request();

            let resp: Vec<Address> = test::read_response_json(&mut app, req).await;
            assert_eq!(resp.len(), 4);
        })
        .await
    }

    #[actix_rt::test]
    async fn test_get_addresses_postcode_and_number() {
        run_test(async {
            let mut app = test::init_service(
                App::new()
                    .data(POOL.clone())
                    .route("/addresses", web::get().to(addresses))
            )
            .await;

            create_test_set().await;

            let req = test::TestRequest::get()
                .uri("/addresses?postcode=2222AA&number=1")
                .to_request();

            let resp: Vec<Address> = test::read_response_json(&mut app, req).await;
            assert_eq!(resp.len(), 1);
            assert_eq!(resp[0].postcode, "2222AA");
            assert_eq!(resp[0].number, "1");

            let req = test::TestRequest::get()
                .uri("/addresses?postcode=2222AA&number=2")
                .to_request();

            let resp: Vec<Address> = test::read_response_json(&mut app, req).await;
            assert_eq!(resp.len(), 3);
            assert_eq!(resp[0].postcode, "2222AA");
            assert_eq!(resp[0].number, "2");
            assert_eq!(resp[1].postcode, "2222AA");
            assert_eq!(resp[1].number, "2A");
            assert_eq!(resp[2].postcode, "2222AA");
            assert_eq!(resp[2].number, "2B");

            let req = test::TestRequest::get()
                .uri("/addresses?postcode=2222AA&number=2A")
                .to_request();

            let resp: Vec<Address> = test::read_response_json(&mut app, req).await;
            assert_eq!(resp.len(), 1);
            assert_eq!(resp[0].postcode, "2222AA");
            assert_eq!(resp[0].number, "2A");

            let req = test::TestRequest::get()
                .uri("/addresses?postcode=2222AA&number=2B")
                .to_request();

            let resp: Vec<Address> = test::read_response_json(&mut app, req).await;
            assert_eq!(resp.len(), 1);
            assert_eq!(resp[0].postcode, "2222AA");
            assert_eq!(resp[0].number, "2B");
        })
        .await
    }

    async fn create_test_set() {
        web::block(|| {
            create_or_update_addresses(
                &POOL.get().unwrap(),
                &[
                    AddressRecord {
                        lat: 2.0,
                        lon: 1.0,
                        number: "1".to_string(),
                        street: "Street".to_string(),
                        city: "City".to_string(),
                        region: "Region".to_string(),
                        postcode: "2222AA".to_string()
                    },
                    AddressRecord {
                        lat: 3.0,
                        lon: 2.0,
                        number: "2".to_string(),
                        street: "Street".to_string(),
                        city: "City".to_string(),
                        region: "Region".to_string(),
                        postcode: "2222AA".to_string()
                    },
                    AddressRecord {
                        lat: 4.0,
                        lon: 3.0,
                        number: "2A".to_string(),
                        street: "Street".to_string(),
                        city: "City".to_string(),
                        region: "Region".to_string(),
                        postcode: "2222AA".to_string()
                    },
                    AddressRecord {
                        lat: 5.0,
                        lon: 4.0,
                        number: "2B".to_string(),
                        street: "Street".to_string(),
                        city: "City".to_string(),
                        region: "Region".to_string(),
                        postcode: "2222AA".to_string()
                    },
                ]
            )
        })
        .await
        .expect("Error creating tests data");
    }
}
