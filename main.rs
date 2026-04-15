use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use std::time::{Duration, Instant};

struct Entry {
    value: String,
    expires_at: Option<Instant>, // None = lives forever, Some = has a deadline
}

type Store = Arc<Mutex<HashMap<String, Entry>>>;

async fn handle_connection(mut socket: TcpStream, store: Store) -> io::Result<()> {
    let mut buf: Vec<u8> = Vec::new();
    loop {
        let mut read_buf = [0u8; 512];
        match socket.read(&mut read_buf).await {
            Ok(0) => return Ok(()),
            Ok(n) => {
                buf.extend_from_slice(&read_buf[..n]);
                if let Some((command, args)) = parse_resp(&buf) {
                    let response = handle_command(&command, args, &store).await;
                    socket.write_all(&response).await?;
                    buf.clear();
                }
            }
            Err(_) => return Ok(()),
        }
    }
}

async fn handle_command(command: &str, args: Vec<Vec<u8>>, store: &Store) -> Vec<u8> {
    match command.to_lowercase().as_str() {
        "ping" => b"+PONG\r\n".to_vec(),

        "echo" => {
            if let Some(arg) = args.first() {
                format_bulk_string(arg)
            } else {
                b"-ERR wrong number of arguments for 'wcho' command\r\n".to_vec()
            }
        }

        "set" => {
            //SET needs 2 args: key and value
            if args.len() < 2 {
                return b"-ERR wrong number of arguments for 'set' command\r\n".to_vec();
            }
            let key = String::from_utf8_lossy(&args[0]).to_string();
            let value = String::from_utf8_lossy(&args[1]).to_string();

            let expires_at = if args.len() >= 4 {
                let option = String::from_utf8_lossy(&args[2]).to_uppercase();
                let amount: u64 = String::from_utf8_lossy(&args[3])
                    .parse()
                    .unwrap_or(0);

                match option.as_str() {
                    "PX" => Some(Instant::now() + Duration::from_millis(amount)),
                    "EX" => Some(Instant::now() + Duration::from_secs(amount)),
                    _    => None,
                }
            } else {
                None // no expiry
            };

            store.lock().await.insert(key, Entry { value, expires_at });

            b"+OK\r\n".to_vec()

        }

        "get" => {
            if args.len() < 1 {
                return b"-ERRwrong number of arguments for 'get' command\r\n".to_vec();
            }
            let key = String::from_utf8_lossy(&args[0]).to_string();

            // Lock, read, release
            let mut store = store.lock().await;
            
            match store.get(&key) {
                Some(entry) => {
                    // Check if this key has an expiry AND if that time has passed
                    if let Some(expires_at) = entry.expires_at {
                        if Instant::now() > expires_at {
                            // Key is expired — delete it and return null
                            store.remove(&key);
                            return b"$-1\r\n".to_vec();
                        }
                    }
                        // Key exists and is not expired — return the value
                        format_bulk_string(entry.value.as_bytes())
                    }
                    None => b"$-1\r\n".to_vec(), // null bulk string = key not found
                }
            }

        _ => b"-ERR unknown command\r\n".to_vec(),
    }
}

fn parse_resp(buf: &[u8]) -> Option<(String, Vec<Vec<u8>>)> {
    if buf.is_empty() {
        return None;
    }

    if buf[0] == b'*' {
        parse_array(buf)
    } else {
        None
    }
}

fn parse_array(buf: &[u8]) -> Option<(String, Vec<Vec<u8>>)> {
    let mut pos = 1;
    let array_len = parse_integer(buf, &mut pos)? as usize;

    let mut args = Vec::new();
    for _ in 0..array_len {
        if pos >= buf.len() {
            return None;
        }

        if buf[pos] != b'$' {
            return None;
        }
        pos += 1;

        let len = parse_integer(buf, &mut pos)? as usize;
        if pos + len + 1 >= buf.len() {
            return None;
        }
        if buf[pos + len] != b'\r' || buf[pos + len + 1] != b'\n' {
            return None;
        }
        let arg = buf[pos..pos + len].to_vec();
        args.push(arg);
        pos += len + 2;
    }

    let command = String::from_utf8(args[0].clone()).ok()?;
    let rest = args[1..].to_vec();
    Some((command, rest))
}

fn parse_integer(buf: &[u8], pos: &mut usize) -> Option<i64> {
    let start = *pos;
    while *pos < buf.len() && buf[*pos] != b'\r' {
        *pos += 1;
    }
    if *pos + 1 > buf.len() || buf[*pos] != b'\r' || buf[*pos + 1] != b'\n' {
        return None;
    }
    let s = std::str::from_utf8(&buf[start..*pos]).ok()?;
    let val = s.parse().ok()?;
    *pos += 2;
    Some(val)
}

fn format_bulk_string(s: &[u8]) -> Vec<u8> {
    format!("${}\r\n{}\r\n", s.len(), String::from_utf8_lossy(s)).into_bytes()
}

#[tokio::main]
async fn main() -> io::Result<()> {
    println!("Logs from your program will appear here!");
    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    // store shared by everyone
    let store: Store = Arc::new(Mutex::new(HashMap::new()));

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                // cloning the Arc (not the HashMap)
                // Both task point to the same data
                let store = Arc::clone(&store);
                tokio::task::spawn(handle_connection(stream, store));
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
