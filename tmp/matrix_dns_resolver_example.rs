// Example usage of Matrix DNS resolver for federation server discovery
// This demonstrates how to properly resolve Matrix server names according to the specification

use std::sync::Arc;
use reqwest::Client;
use trust_dns_resolver::TokioResolver;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts};

// Import the Matrix DNS resolver components
use crate::federation::dns_resolver::{MatrixDnsResolver, ResolvedServer, ResolutionMethod};
use crate::federation::well_known_client::WellKnownClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create HTTP client for well-known requests
    let http_client = Arc::new(Client::new());
    
    // Create well-known client
    let well_known_client = Arc::new(WellKnownClient::new(http_client));
    
    // Create Matrix DNS resolver
    let dns_resolver = MatrixDnsResolver::new(well_known_client)?;

    // Example server names to resolve
    let server_names = vec![
        "matrix.org",           // Should use well-known delegation
        "example.com:8448",     // Explicit port
        "192.168.1.1:8448",     // IP literal
        "[::1]:8448",           // IPv6 literal
        "test.matrix.org",      // May use SRV records
    ];

    for server_name in server_names {
        println!("\n=== Resolving: {} ===", server_name);
        
        match dns_resolver.resolve_server(server_name).await {
            Ok(resolved) => {
                print_resolved_server(&resolved);
                
                // Example of how to use the resolved server for HTTP requests
                let base_url = dns_resolver.get_base_url(&resolved);
                let host_header = dns_resolver.get_host_header(&resolved);
                
                println!("Base URL: {}", base_url);
                println!("Host header: {}", host_header);
                
                // Example federation request URL
                let federation_url = format!("{}/_matrix/federation/v1/version", base_url);
                println!("Federation URL: {}", federation_url);
            }
            Err(e) => {
                eprintln!("Failed to resolve {}: {}", server_name, e);
            }
        }
    }

    Ok(())
}

fn print_resolved_server(resolved: &ResolvedServer) {
    println!("✅ Resolved successfully:");
    println!("  IP Address: {}", resolved.ip_address);
    println!("  Port: {}", resolved.port);
    println!("  Host Header: {}", resolved.host_header);
    println!("  TLS Hostname: {}", resolved.tls_hostname);
    println!("  Resolution Method: {:?}", resolved.resolution_method);
    
    match resolved.resolution_method {
        ResolutionMethod::IpLiteral => {
            println!("  📍 Used IP literal from server name");
        }
        ResolutionMethod::ExplicitPort => {
            println!("  🔌 Used explicit port from server name");
        }
        ResolutionMethod::WellKnownDelegation => {
            println!("  🌐 Used well-known delegation (/.well-known/matrix/server)");
        }
        ResolutionMethod::SrvMatrixFed => {
            println!("  📡 Used _matrix-fed._tcp SRV record (Matrix v1.8+)");
        }
        ResolutionMethod::SrvMatrixLegacy => {
            println!("  📡 Used legacy _matrix._tcp SRV record (deprecated)");
        }
        ResolutionMethod::FallbackPort8448 => {
            println!("  🔄 Used fallback to hostname:8448");
        }
    }
}

// Example of integrating with existing federation code
async fn make_federation_request(
    dns_resolver: &MatrixDnsResolver,
    server_name: &str,
    path: &str,
) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
    // Resolve the server according to Matrix specification
    let resolved = dns_resolver.resolve_server(server_name).await?;
    
    // Create HTTP client
    let client = reqwest::Client::new();
    
    // Build the request URL using the resolved IP and port
    let base_url = dns_resolver.get_base_url(&resolved);
    let url = format!("{}{}", base_url, path);
    
    // Make the request with proper Host header
    let response = client
        .get(&url)
        .header("Host", dns_resolver.get_host_header(&resolved))
        .header("User-Agent", "matryx-server/1.0")
        .send()
        .await?;
    
    Ok(response)
}

// Example SRV record setup for testing
// Add these to your DNS zone file for testing:
//
// _matrix-fed._tcp.example.com. 300 IN SRV 10 5 8448 matrix.example.com.
// _matrix._tcp.example.com.     300 IN SRV 10 5 8448 matrix.example.com.
// matrix.example.com.           300 IN A    192.168.1.100
//
// Well-known setup for testing:
// https://example.com/.well-known/matrix/server
// {
//   "m.server": "matrix.example.com:8448"
// }