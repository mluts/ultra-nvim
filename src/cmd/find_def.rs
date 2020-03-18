use crate::cmd;
use crate::nrepl;
use crate::nrepl::ops;
use crate::nrepl::session;
use crate::nrepl::NreplOp;
use clap::{clap_app, App, ArgMatches};

struct Opts {
    ns: String,
    symbol: String,
}

impl Opts {
    fn parse(matches: &ArgMatches) -> Opts {
        let ns = matches.value_of("NS").unwrap().to_string();
        let symbol = matches.value_of("SYMBOL").unwrap().to_string();

        Opts { ns, symbol }
    }
}

pub fn app<'a, 'b>() -> App<'a, 'b> {
    clap_app!(find_def =>
        (about: "Shows position of ns/symbol")
        (@arg NS: +required "NS")
        (@arg SYMBOL: +required "SYMBOL")
    )
}

pub fn run(matches: &ArgMatches, nrepl_stream: &nrepl::NreplStream) {
    let opts = Opts::parse(matches);
    let session = cmd::die_if_err(session::get_existing_session_id(nrepl_stream));
    let op = ops::Info::new(session, opts.ns, opts.symbol);
    let res = cmd::die_if_err(op.send(nrepl_stream));

    if let Some(res) = res {
        match res {
            ops::InfoResponseType::Ns(res) => {
                cmd::print_parseable(&vec![
                    ("IS-NS", "TRUE"),
                    ("LINE", &res.line.to_string()),
                    ("FILE", &res.file),
                    ("RESOURCE", &res.resource),
                ]);
            }

            ops::InfoResponseType::Symbol(res) => {
                cmd::print_parseable(&vec![
                    ("IS-SYMBOL", "TRUE"),
                    ("LINE", &res.line.to_string()),
                    ("COLUMN", &res.col.unwrap().to_string()),
                    ("FILE", &res.file),
                    ("RESOURCE", &res.resource),
                ]);
            }
        }
    } else {
        cmd::print_parseable(&vec![("IS-EMPTY", "TRUE")]);
    }
}
