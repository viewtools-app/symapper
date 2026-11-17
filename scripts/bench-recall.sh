#!/usr/bin/env bash
# Recall benchmark. Timing uses Perl Time::HiRes on macOS and Linux.
# TSV and map paths must be single-line text without tabs; CSV strings are quoted.
set -euo pipefail
set -f
export LC_ALL=C
usage() {
    printf '%s\n' \
        'Usage: bench-recall.sh [--set tune|held|t15|all] <tasks.tsv> <out-dir>' \
        'Default set: tune. BENCH_REPOS is required; SYMAPPER_BIN defaults to symapper.' \
        'Timing requires perl with Time::HiRes. Use a fresh output directory per run.'
}
die() { printf 'bench-recall: %s\n' "$*" >&2; exit 2; }
selected=tune
if [ "${1:-}" = --help ]; then usage; exit 0; fi
if [ "${1:-}" = --set ]; then
    [ "$#" -ge 2 ] || die '--set requires a value'
    selected=$2
    shift 2
fi
case "$selected" in tune|held|t15|all) ;; *) die 'unknown set' ;; esac
[ "$#" -eq 2 ] || { usage >&2; exit 2; }
tasks=$1
[ -r "$tasks" ] || die 'cannot read tasks TSV'
[ -n "${BENCH_REPOS:-}" ] || die 'BENCH_REPOS is required'
[ -d "$BENCH_REPOS" ] || die 'BENCH_REPOS must be a directory'
repos=$(cd "$BENCH_REPOS" && pwd -P)
bin=${SYMAPPER_BIN:-symapper}
case "$bin" in */*) bin=$(cd "${bin%/*}" && pwd -P)/${bin##*/} ;; esac
command -v "$bin" >/dev/null 2>&1 || die 'SYMAPPER_BIN is not executable'
command -v perl >/dev/null 2>&1 || die 'perl with Time::HiRes is required'
perl -MTime::HiRes -e 'exit 0' || die 'Perl Time::HiRes is required'
# Validate before creating output or cloning. Ignore comments and accept CRLF.
awk -F '\t' '
    /^#/ { next }
    { sub(/\r$/, "") }
    !header {
        if ($0 != "id\trepo\tcommit\tfocus\ttruth\tnew\tk_base\tset") exit 2
        header=1; next
    }
    {
        if (NF != 8 || $1 == "" || $2 !~ /^[A-Za-z0-9_.-]+$/ ||
            $2 == "." || $2 == ".." || $3 !~ /^[A-Fa-f0-9]+$/ ||
            $4 !~ /[^ ]/ || ($7 != "" && $7 !~ /^[0-9]+$/) ||
            $8 !~ /^(tune|held|t15)$/) exit 2
        safe=$1; gsub(/[^A-Za-z0-9_.-]/, "_", safe)
        if (safe == "." || safe == ".." || seen[safe]++) exit 2
    }
    END { if (!header) exit 2 }
' "$tasks" || die 'bad TSV header or task row'
mkdir -p "$2"
out=$(cd "$2" && pwd -P)
[ ! -e "$out/results.csv" ] || die 'results.csv already exists; use a fresh output directory'
mkdir -p "$out/clones" "$out/cache" "$out/maps" "$out/data"
printf '%s\n' 'task,commit,mode,focus,truth_existing,truth_nontest,truth_new,hits,hits_nontest,recall,paths_in_map,K,hit_rank,hit_line,hit_kind,wall_s,cold_s,hit_files,new_files,missed' > "$out/results.csv"
: > "$out/data/totals.tsv"
# Ignore user/system Git helpers and hooks; never execute code from a benchmark repo.
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR
unset GIT_CONFIG_PARAMETERS GIT_EXTERNAL_DIFF
source_paths() {
    awk '/\.(rs|go|ts|tsx|js|mjs|py|swift)$/ && !/\.d\.ts$/'
}
map_paths() {
    awk '
        FILENAME == ARGV[1] { tracked[$0]=1; next }
        {
            path=$0; sig=sub(/:$/, "", path)
            if (path in tracked)
                print path "\t" ++rank "\t" FNR "\t" (sig ? "sig" : "name")
        }
    ' "$tracked" "$1" > "$2"
}
run_map() {
    local dest=$1 seconds=$2 budget=$3
    shift 3
    local start end
    start=$(perl -MTime::HiRes=time -e 'printf "%.9f", time')
    SYMAPPER_CACHE_DIR="$cache" "$bin" "$clone" \
        --tokens "$budget" "${focus_args[@]}" "$@" > "$dest"
    end=$(perl -MTime::HiRes=time -e 'printf "%.9f", time')
    awk -v start="$start" -v end="$end" 'BEGIN { printf "%.6f\n", end-start }' > "$seconds"
}
# A non-whitespace separator preserves empty truth/new/K fields with Bash 3.2 read.
awk -F '\t' -v selected="$selected" '
    /^#/ { next }
    { sub(/\r$/, "") }
    !header { header=1; next }
    selected == "all" || $8 == selected { gsub(/\t/, "\034"); print }
' "$tasks" |
while IFS=$'\034' read -r id repo commit focus truth new k_base _task_set; do
    safe=$(printf '%s' "$id" | tr -c 'A-Za-z0-9._-' '_')
    clone=$out/clones/$safe
    cache=$out/cache/$safe
    data=$out/data/$safe
    [ ! -e "$clone" ] || die 'task clone already exists; use a fresh output directory'
    [ -d "$repos/$repo" ] || die "missing repository for selected task $id"
    git -c core.hooksPath=/dev/null clone -q --shared --no-checkout "$repos/$repo" "$clone"
    git -C "$clone" -c core.hooksPath=/dev/null -c core.fsmonitor=false \
        -c advice.detachedHead=false checkout -q "$commit^"
    mkdir -p "$cache" "$data"
    tracked=$data/tracked.txt
    git -C "$clone" -c core.quotePath=false -c core.fsmonitor=false ls-files > "$tracked"
    : > "$data/truth.txt"
    : > "$data/new.txt"
    if [ -z "$truth" ]; then
        git -C "$clone" -c core.quotePath=false diff --no-ext-diff --no-textconv \
            --name-only "$commit^" "$commit" -- > "$data/changed.txt"
        source_paths < "$data/changed.txt" > "$data/sources.txt"
        while IFS= read -r path; do
            if git -C "$clone" cat-file -e "$commit^:$path" 2>/dev/null; then
                printf '%s\n' "$path" >> "$data/truth.txt"
            else
                printf '%s\n' "$path" >> "$data/new.txt"
            fi
        done < "$data/sources.txt"
    else
        printf '%s\n' "$truth" | tr ';' '\n' | source_paths | sort -u > "$data/truth.txt"
        printf '%s\n' "$new" | tr ';' '\n' | source_paths | sort -u > "$data/new.txt"
    fi
    focus_args=()
    for word in $focus; do focus_args+=(--focus "$word"); done
    run_map "$out/maps/$safe.cold1500.txt" "$data/cold_s" 1500 --no-cache
    run_map "$out/maps/$safe.seed1500.txt" "$data/seed_s" 1500
    run_map "$out/maps/$safe.sym1500.txt" "$data/warm_s" 1500
    run_map "$out/maps/$safe.sym3000.txt" "$data/large_s" 3000
    map_paths "$out/maps/$safe.sym1500.txt" "$data/sym1500.tsv"
    map_paths "$out/maps/$safe.sym3000.txt" "$data/sym3000.tsv"
    : > "$data/grep-raw.tsv"
    for word in $focus; do
        if git -C "$clone" -c core.quotePath=false -c core.fsmonitor=false grep \
            --no-textconv -c -i -w -e "$word" -- \
            '*.rs' '*.go' '*.ts' '*.tsx' '*.js' '*.mjs' '*.py' '*.swift' \
            ':!*.d.ts' > "$data/grep-word.txt"; then
            awk -v word="$word" '{
                path=$0; sub(/:[0-9]+$/, "", path)
                count=substr($0, length(path)+2)
                print word "\t" path "\t" count
            }' "$data/grep-word.txt" >> "$data/grep-raw.tsv"
        else
            rc=$?
            [ "$rc" -eq 1 ] || exit "$rc"
        fi
    done
    awk -F '\t' '
        !seen[$1 SUBSEP $2]++ { words[$2]++; matches[$2]+=$3 }
        END { for (path in words) print path "\t" words[path] "\t" matches[path] }
    ' "$data/grep-raw.tsv" |
        sort -t $'\t' -k2,2nr -k3,3nr -k1,1 > "$data/grep-ranked.tsv"
    # Emit CSV and a numeric totals stream, so quoted CSV needs no parser.
    awk -F '\t' -v task="$id" -v commit="$commit" -v focus="$focus" \
        -v base="$k_base" -v data="$data" -v totals="$out/data/totals.tsv" '
        function source(path) {
            return path ~ /\.(rs|go|ts|tsx|js|mjs|py|swift)$/ && path !~ /\.d\.ts$/
        }
        function testfile(path,    n,parts) {
            if (path ~ /(^|\/)(tests|__tests__)\//) return 1
            n=split(path,parts,"/")
            return parts[n] ~ /(test|spec)/
        }
        function csv(value,    escaped) {
            escaped=value; gsub(/"/, "\"\"", escaped)
            return "\"" escaped "\""
        }
        function append(list,value) { return list == "" ? value : list ";" value }
        function readmap(mode,    line,parts,p) {
            while ((getline line < (data "/" mode ".tsv")) > 0) {
                split(line,parts,"\t"); p=parts[1]
                paths[mode]++
                if (!((mode SUBSEP p) in ranks)) {
                    ranks[mode,p]=parts[2]; lines[mode,p]=parts[3]; kinds[mode,p]=parts[4]
                    if (source(p)) sizes[mode]++
                }
            }
            close(data "/" mode ".tsv")
        }
        function emit(mode,limit,wall,cold,    i,p,hits,hn,r,l,k,hf,miss,share,recall,pc,kval) {
            hits=0; hn=0; share=0; r=""; l=""; k=""; hf=""; miss=""
            for (i=1;i<=nt;i++) {
                p=truth[i]
                if (((mode SUBSEP p) in ranks) && (limit < 0 || ranks[mode,p] <= limit)) {
                    hits++; if (!testfile(p)) hn++
                    hf=append(hf,p); r=append(r,ranks[mode,p])
                    if (mode ~ /^sym/) {
                        l=append(l,lines[mode,p]); k=append(k,kinds[mode,p])
                        if (kinds[mode,p]=="sig") share++
                    }
                } else miss=append(miss,p)
            }
            recall=nt ? sprintf("%.6f",hits/nt) : ""
            if (mode ~ /^sym/) { pc=paths[mode]+0; kval=sizes[mode]+0 }
            else { pc=ng < limit ? ng : limit; kval=limit }
            print csv(task) "," csv(commit) "," mode "," csv(focus) "," nt+0 "," nnt+0 "," nn+0 "," \
                hits "," hn "," recall "," pc "," kval "," csv(r) "," csv(l) "," csv(k) "," \
                wall "," cold "," csv(hf) "," csv(newfiles) "," csv(miss)
            print mode "\t" nt+0 "\t" hits "\t" share "\t" r "\t" cold "\t" wall >> totals
        }
        BEGIN {
            while ((getline p < (data "/truth.txt"))>0) {
                if (p!="" && !truth_seen[p]++) { truth[++nt]=p; if (!testfile(p)) nnt++ }
            }
            while ((getline p < (data "/new.txt"))>0) {
                if (p!="" && !new_seen[p]++) { nn++; newfiles=append(newfiles,p) }
            }
            readmap("sym1500"); readmap("sym3000")
            while ((getline line < (data "/grep-ranked.tsv"))>0) {
                split(line,parts,"\t"); p=parts[1]; ng++
                ranks["grep",p]=ng; ranks["grepnewk",p]=ng
            }
            getline cold < (data "/cold_s")
            getline warm < (data "/warm_s")
            getline large < (data "/large_s")
            newk=sizes["sym1500"]+0
            if (base=="") base=newk
            emit("sym1500",-1,warm,cold)
            emit("sym3000",-1,large,"")
            emit("grep",base,"","")
            emit("grepnewk",newk,"","")
        }
    ' >> "$out/results.csv"
done
# Portable insertion-sort median; no GNU awk asort dependency.
awk -F '\t' -v selected="$selected" '
    function median(group,    n,i,j,v,a) {
        n=count[group]; if (!n) return "-"
        for (i=1;i<=n;i++) a[i]=values[group,i]+0
        for (i=2;i<=n;i++) {
            v=a[i]; j=i-1
            while (j>0 && a[j]>v) { a[j+1]=a[j]; j-- }
            a[j+1]=v
        }
        return n%2 ? a[(n+1)/2] : (a[n/2]+a[n/2+1])/2
    }
    function push(group,value) { values[group,++count[group]]=value }
    {
        mode=$1; tasks[mode]++; truth[mode]+=$2; hits[mode]+=$3; sig[mode]+=$4
        n=split($5,ranks,";")
        for (i=1;i<=n;i++) if (ranks[i]!="") push(mode,ranks[i])
        if (mode=="sym1500") { push("cold",$6); push("warm",$7) }
    }
    END {
        printf "set=%s\n",selected
        print "mode tasks truth micro_recall median_hit_rank signature_share cold_median_s warm_median_s"
        split("sym1500 sym3000 grep grepnewk",modes," ")
        for (i=1;i<=4;i++) {
            mode=modes[i]
            recall=truth[mode] ? sprintf("%.6f",hits[mode]/truth[mode]) : "-"
            share=mode ~ /^sym/ && hits[mode] ? sprintf("%.6f",sig[mode]/hits[mode]) : "-"
            printf "%s %d %d %s %s %s %s %s\n", mode,tasks[mode],truth[mode],recall,median(mode),share, \
                (mode=="sym1500" ? median("cold") : "-"), (mode=="sym1500" ? median("warm") : "-")
        }
    }
' "$out/data/totals.tsv"
