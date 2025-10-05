use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    transport::smtp::authentication::Credentials,
    message::{Mailbox, MultiPart, header::ContentType},
};
use tera::{Tera, Context};
use std::sync::Arc;
use crate::config::server_config::EmailConfig;

pub struct EmailService {
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    templates: Arc<Tera>,
    from_address: Mailbox,
    server_name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("SMTP error: {0}")]
    Smtp(#[from] lettre::transport::smtp::Error),
    #[error("Template error: {0}")]
    Template(#[from] tera::Error),
    #[error("Email address parse error: {0}")]
    AddressParse(#[from] lettre::address::AddressError),
    #[error("Email building error: {0}")]
    EmailBuild(#[from] lettre::error::Error),
}

impl EmailService {
    pub fn new(config: &EmailConfig, server_name: String) -> Result<Self, EmailError> {
        // Build SMTP transport with credentials
        let creds = Credentials::new(
            config.smtp_username.clone(),
            config.smtp_password.clone(),
        );
        
        let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_server)?
            .port(config.smtp_port)
            .credentials(creds)
            .build();
        
        // Load Tera templates from templates/email/ directory
        let mut tera = Tera::new("templates/email/**/*.{html,txt}")?;
        tera.autoescape_on(vec![".html"]);  // Auto-escape HTML, not text
        
        // Parse from address
        let from_address = config.from_address.parse()?;
        
        Ok(Self {
            mailer,
            templates: Arc::new(tera),
            from_address,
            server_name,
        })
    }
    
    pub async fn send_verification_email(
        &self,
        to_email: &str,
        token: &str,
        session_id: &str,
    ) -> Result<(), EmailError> {
        let mut context = Context::new();
        context.insert("token", token);
        context.insert("session_id", session_id);
        context.insert("server_name", &self.server_name);
        context.insert("verification_url", &format!("{}/_matrix/client/v3/account/3pid/email/submit_token?token={}&sid={}", 
            self.server_name, token, session_id));
        
        let html_body = self.templates.render("verification.html", &context)?;
        let text_body = self.templates.render("verification.txt", &context)?;
        
        self.send_multipart_email(to_email, "Verify your email address", html_body, text_body).await
    }
    
    pub async fn send_password_reset_email(
        &self,
        to_email: &str,
        token: &str,
        session_id: &str,
    ) -> Result<(), EmailError> {
        let mut context = Context::new();
        context.insert("token", token);
        context.insert("session_id", session_id);
        context.insert("server_name", &self.server_name);
        context.insert("reset_url", &format!("{}/_matrix/client/v3/account/password/email/submit_token?token={}&sid={}", 
            self.server_name, token, session_id));
        
        let html_body = self.templates.render("password_reset.html", &context)?;
        let text_body = self.templates.render("password_reset.txt", &context)?;
        
        self.send_multipart_email(to_email, "Reset your password", html_body, text_body).await
    }
    
    pub async fn send_moderator_notification(
        &self,
        admin_email: &str,
        reporter_id: &str,
        reported_user_id: &str,
        reason: &str,
    ) -> Result<(), EmailError> {
        let mut context = Context::new();
        context.insert("reporter_id", reporter_id);
        context.insert("reported_user_id", reported_user_id);
        context.insert("reason", reason);
        context.insert("server_name", &self.server_name);
        context.insert("timestamp", &chrono::Utc::now().to_rfc3339());
        
        let html_body = self.templates.render("moderator_notification.html", &context)?;
        let text_body = self.templates.render("moderator_notification.txt", &context)?;
        
        self.send_multipart_email(admin_email, 
            &format!("User Report: {} reported {}", reporter_id, reported_user_id), 
            html_body, text_body).await
    }
    
    async fn send_multipart_email(
        &self,
        to_email: &str,
        subject: &str,
        html_body: String,
        text_body: String,
    ) -> Result<(), EmailError> {
        let to_address: Mailbox = to_email.parse()?;
        
        let email = Message::builder()
            .from(self.from_address.clone())
            .to(to_address)
            .subject(subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text_body)
                    )
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html_body)
                    )
            )?;
        
        self.mailer.send(email).await?;
        tracing::info!("Email sent successfully to {}", to_email);
        Ok(())
    }
}
