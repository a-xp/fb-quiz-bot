mod types;

#[derive(PartialEq, Debug, Clone)]
struct AdminUser {
    pub login: String,
    pub secret: String,
}
