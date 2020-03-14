use crate::bencode;
use crate::bencode::json as bencode_json;
use bendy::encoding::{Error as BError, SingleItemEncoder, ToBencode};
use serde_json::error as json_error;
use serde_json::value::Value as JsonValue;
use std::collections::HashMap;
use std::fmt;
use std::io::{BufReader, BufWriter, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

#[derive(Debug)]
pub enum Error {
    ConnectionLost,
    BencodeEncodeError(BError),
    BencodeDecodeError(bencode::Error),
    UnexpectedBencodeObject(bencode::Object),
    IOError(std::io::Error),
    BadBencodeString(std::string::FromUtf8Error),
    DuplicatedKeyError(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Nrepl Error: {}",
            match self {
                Error::ConnectionLost => format!("Connection lost!"),
                Error::BencodeDecodeError(err) => format!("Bencode decode error: {:?}", err),
                Error::IOError(io_err) => format!("IO Error: {}", io_err),
                Error::BencodeEncodeError(berr) => format!("Bencode encode error: {}", berr),
                Error::UnexpectedBencodeObject(o) => format!("Unexpected bencode object: {:?}", o),
                Error::BadBencodeString(utf8err) => format!("Bad string in bencode: {}", utf8err),
                Error::DuplicatedKeyError(k) =>
                    format!("Key {} was duplicated in response dict", k), // Error::BadResponse(s) => format!()
            }
        )
    }
}

pub struct NreplStream {
    tcp: TcpStream,
}

pub struct Op {
    name: String,
    args: Vec<(String, String)>,
}

impl Op {
    pub fn new(name: String, args: Vec<(String, String)>) -> Op {
        Op { name, args }
    }
}

pub type Resp = HashMap<String, bencode::Object>;

pub fn to_json_string(resp: &Resp) -> Result<String, json_error::Error> {
    let mut hm: HashMap<String, JsonValue> = HashMap::new();

    for (k, v) in resp.iter() {
        hm.insert(k.to_string(), bencode_json::to_json_val(v).unwrap());
    }

    serde_json::to_string(&hm)
}

impl ToBencode for Op {
    const MAX_DEPTH: usize = 3;

    fn encode(&self, encoder: SingleItemEncoder) -> Result<(), BError> {
        encoder.emit_dict(|mut e| {
            let mut pairs: Vec<(&str, &str)> = vec![];

            pairs.push(("op", &self.name));

            for (argname, argval) in self.args.iter() {
                pairs.push((argname, argval));
            }

            pairs.sort();

            for (argname, argval) in pairs.into_iter() {
                e.emit_pair(&argname.clone().as_bytes(), argval)?;
            }

            Ok(())
        })?;
        Ok(())
    }
}

fn is_final_resp(resp: &Resp) -> bool {
    resp.contains_key("status")
}

impl NreplStream {
    pub fn connect_timeout(addr: &SocketAddr) -> Result<NreplStream, Error> {
        TcpStream::connect_timeout(addr, Duration::new(3, 0))
            .and_then(|t| {
                t.set_nonblocking(false)?;
                t.set_read_timeout(Some(Duration::new(5, 0)))?;
                Ok(t)
            })
            .map(|s| NreplStream { tcp: s })
            .map_err(|e| Error::IOError(e))
    }

    fn send_op(&self, op: &Op) -> Result<(), Error> {
        let mut bw = BufWriter::new(&self.tcp);
        let bencode = op.to_bencode().map_err(|e| Error::BencodeEncodeError(e))?;
        bw.write(&bencode).map_err(|e| Error::IOError(e))?;
        Ok(())
    }

    fn read_resp(&self) -> Result<Resp, Error> {
        let mut br = BufReader::new(&self.tcp);
        let mut decoder = bencode::Decoder::new(&mut br);

        match decoder
            .read_object()
            .map(|o| o.expect("Wasn't able to read response to the end"))
        {
            Ok(bencode::Object::Dict(pairs)) => {
                let mut resp: Resp = HashMap::new();

                for (k, v) in pairs.into_iter() {
                    let k_str =
                        String::from_utf8(k.to_vec()).map_err(|e| Error::BadBencodeString(e))?;
                    if resp.contains_key(&k_str) {
                        return Err(Error::DuplicatedKeyError(k_str));
                    }

                    resp.insert(k_str, v);
                }
                Ok(resp)
            }
            Ok(o) => Err(Error::UnexpectedBencodeObject(o)),
            Err(e) => Err(Error::BencodeDecodeError(e)),
        }
    }

    pub fn op(&self, op: &Op) -> Result<Vec<Resp>, Error> {
        let mut resps: Vec<Resp> = vec![];

        self.send_op(op)?;

        loop {
            let resp = self.read_resp()?;
            let is_final = is_final_resp(&resp);

            resps.push(resp);

            if is_final {
                break;
            }
        }

        Ok(resps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bencode;
    use std::collections::HashMap;
    use std::iter::FromIterator;

    #[test]
    fn final_resp_test() {
        let final_resp = HashMap::from_iter(
            vec![("status".to_string(), bencode::Object::BBytes(vec![]))].into_iter(),
        );

        let not_final_resp = HashMap::from_iter(
            vec![("foo".to_string(), bencode::Object::BBytes(vec![]))].into_iter(),
        );

        assert!(is_final_resp(&final_resp));
        assert!(!is_final_resp(&not_final_resp));
    }
}
