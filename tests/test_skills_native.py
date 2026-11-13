#!/usr/bin/env python3
"""Explicit private native integration; no installed harness/config writes.

Run with --tambo /owned/candidate --quipu /owned/quipu --render-shapes /pinned/render.ttl.
The caller supplies actual executables; this script never downloads or installs them.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import tarfile
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tambo', required=True, type=Path)
    parser.add_argument('--quipu', required=True, type=Path)
    parser.add_argument('--render-shapes', required=True, type=Path)
    parser.add_argument('--demo', action='store_true', help='Print observed local artifacts and CLI outcomes')
    args = parser.parse_args()
    tambo, quipu, shapes = [p.resolve(strict=True) for p in (args.tambo, args.quipu, args.render_shapes)]
    with tempfile.TemporaryDirectory(prefix='skills-native-') as directory:
        root = Path(directory)

        def run(program, *arguments, success=True):
            result = subprocess.run([str(program), *map(str, arguments)], cwd=root,
                                    text=True, capture_output=True, timeout=90)
            if success and result.returncode:
                raise AssertionError(result.stdout + result.stderr)
            if not success:
                assert result.returncode != 0, result.stdout
            return result

        def dump(path, value):
            path.write_text(json.dumps(value))

        def snapshot():
            return {str(p.relative_to(dest)): (p.stat().st_ino, p.stat().st_mtime_ns,
                    stat.S_IMODE(p.stat().st_mode), p.read_bytes()) for p in dest.rglob('*') if p.is_file()}

        database = root / 'authority.db'
        run(quipu, 'shapes', 'load', 'render', shapes, '--db', database)
        authority_dir = root / 'authority'
        run(quipu, 'share', '--output', authority_dir, '--shapes', 'render',
            '--no-queries', '--destination', 'internal', '--db', database)
        authority = root / 'authority.pendant.tar.gz'
        with tarfile.open(authority, 'w:gz') as archive:
            archive.add(authority_dir, arcname='pendant')
        repo = Path(__file__).resolve().parents[1]
        run(quipu, 'shapes', 'load', 'identifier-policy',
            repo / 'packs/policy/identifier-policy.shapes.ttl', '--db', database)
        run(quipu, 'knot', repo / 'packs/policy/identifier-policy.ttl', '--db', database)
        public_authority_dir = root / 'public-authority'
        run(quipu, 'share', '--output', public_authority_dir, '--shapes', 'render',
            '--shapes', 'identifier-policy', '--no-queries', '--destination', 'outward', '--db', database)
        public_authority = root / 'public-authority.pendant.tar.gz'
        with tarfile.open(public_authority, 'w:gz') as archive:
            archive.add(public_authority_dir, arcname='pendant')
        releases, bundles, expected = [], {}, {}
        for i, name in enumerate(['one', 'two', 'own']):
            source = root / name
            source.mkdir()
            markdown = f'---\nname: {name}\ndescription: fixture\n---\n{{{{ raw }}}}\nUnicode λ\n'.encode()
            (source / 'SKILL.md').write_bytes(markdown)
            (source / 'SKILL.md').chmod(0o644)
            binary = bytes([0, i, 255])
            (source / 'helper').write_bytes(binary)
            (source / 'helper').chmod(0o755)
            (source / 'agents').mkdir()
            agent_metadata = ('interface:\n  display_name: ' + json.dumps(name) + '\n').encode()
            (source / 'agents/openai.yaml').write_bytes(agent_metadata)
            (source / 'agents/openai.yaml').chmod(0o644)
            meta = dict(skill=f'urn:fixture:skill:{name}', name=name, description='fixture skill',
                        repository='https://example.test/fixture', source_path=f'skills/{name}',
                        version='1', render_spec=f'urn:fixture:render:{name}', public=i < 2,
                        harnesses=['claude', 'codex'])
            metadata = root / f'{name}.json'
            dump(metadata, meta)
            output = root / f'{name}-release'
            policy = []
            if meta['public']:
                policy = ['--identifier-policy', repo / 'packs/policy/identifier-policy.ttl',
                          '--identifier-shapes', repo / 'packs/policy/identifier-policy.shapes.ttl']
            result = json.loads(run(tambo, 'skills', 'publish', '--source', source, '--metadata', metadata,
                '--render-shapes', shapes, '--output', output, '--quipu', quipu, *policy).stdout)
            release = result['release']
            if args.demo:
                print(f"publish {name}: outcome={result['outcome']}, public={release['public']}, share={release['pin']['share_id']}")
            assert result['catalogue_registered'] is False
            releases.append(release)
            bundles[release['pin']['share_id']] = str(output / 'bundle.pendant.tar.gz')
            for harness in ['claude', 'codex']:
                expected[f'{harness}/{name}/SKILL.md'] = (markdown, 0o644)
                expected[f'{harness}/{name}/helper'] = (binary, 0o755)
                expected[f'{harness}/{name}/agents/openai.yaml'] = (agent_metadata, 0o644)
            run(tambo, 'skills', 'publish', '--source', source, '--metadata', metadata,
                '--render-shapes', shapes, '--output', output, '--quipu', quipu, success=False)
        profile = dict(id='urn:fixture:profile', public_only=False, harnesses=['claude', 'codex'],
                       pins=[release['pin'] for release in releases])
        catalogue = dict(transaction=1, complete=True, releases=releases)
        profile_path, catalogue_path, bundles_path = [root / (n + '.json') for n in ['profile', 'catalogue', 'bundles']]
        dump(profile_path, profile)
        dump(catalogue_path, catalogue)
        dump(bundles_path, bundles)
        dest = root / 'profile-output'

        def sync(success=True):
            return run(tambo, 'skills', 'sync', '--profile', profile_path, '--catalogue', catalogue_path,
                '--bundles', bundles_path, '--authority', public_authority if profile['public_only'] else authority,
                '--render-shapes', shapes,
                '--dest', dest, '--quipu', quipu, success=success)

        if args.demo:
            print(f'before sync: destination_exists={dest.exists()}')
        first = json.loads(sync().stdout)
        assert first['harness_registration'] is False
        for path, (content, mode) in expected.items():
            assert (dest / path).read_bytes() == content, path
            assert stat.S_IMODE((dest / path).stat().st_mode) == mode, path
        assert json.loads((dest / 'profile.json').read_text())['selection']['profile'] == profile['id']
        if args.demo:
            for harness in ['claude', 'codex']:
                print(f"after sync {harness}: {sorted(p.parent.name for p in (dest / harness).glob('*/SKILL.md'))}")
            print('one/SKILL.md rendered text:', (dest / 'codex/one/SKILL.md').read_text().splitlines()[-2:])
            print('helper mode:', oct(stat.S_IMODE((dest / 'codex/one/helper').stat().st_mode)))
            print('nested agents/openai.yaml preserved:', (dest / 'codex/one/agents/openai.yaml').is_file())
        before = snapshot()
        again = json.loads(sync().stdout)
        assert again['render']['diff']['noop'] is True
        assert snapshot() == before
        if args.demo:
            print('repeat sync native diff.noop:', again['render']['diff']['noop'], '; inode/mtime/bytes unchanged:', snapshot() == before)
        print('native three-skill/two-harness byte+mode+pin+NOOP: PASS')
        for change in ['incomplete', 'wrong-pin', 'public-internal', 'corrupt-bundle']:
            if change == 'incomplete': catalogue['complete'] = False
            elif change == 'wrong-pin': profile['pins'][0] = dict(profile['pins'][0], version='2')
            elif change == 'public-internal': profile['public_only'] = True
            else:
                original = Path(bundles[releases[0]['pin']['share_id']])
                corrupt = root / 'corrupt.pendant.tar.gz'
                corrupt.write_bytes(original.read_bytes()[:64])
                bundles[releases[0]['pin']['share_id']] = str(corrupt)
            dump(profile_path, profile); dump(catalogue_path, catalogue); dump(bundles_path, bundles)
            sync(success=False)
            assert snapshot() == before, change
            catalogue['complete'] = True
            profile['pins'] = [r['pin'] for r in releases]
            profile['public_only'] = False
        print('partial/wrong-pin/public-internal/corrupt share preserve destination: PASS')
        bundles[releases[0]['pin']['share_id']] = str(root / 'one-release/bundle.pendant.tar.gz')
        dump(bundles_path, bundles); dump(profile_path, profile); dump(catalogue_path, catalogue)
        # Metadata alone cannot relabel the verified internal third share.
        catalogue['releases'][2]['public'] = True
        profile['public_only'] = True
        dump(profile_path, profile); dump(catalogue_path, catalogue)
        sync(success=False)
        assert snapshot() == before
        catalogue['releases'][2]['public'] = False
        profile['public_only'] = False
        dump(profile_path, profile); dump(catalogue_path, catalogue)
        print('native internal share stamp defeats forged public metadata: PASS')
        (dest / 'foreign').write_text('preserve')
        before = snapshot()
        sync(success=False)
        assert snapshot() == before
        (dest / 'foreign').unlink()
        profile['pins'] = [r['pin'] for r in releases[:2]]
        profile['public_only'] = True
        dump(profile_path, profile)
        sync()
        assert not (dest / 'claude/own').exists() and not (dest / 'codex/own').exists()
        assert (dest / 'claude/one/SKILL.md').read_bytes() == expected['claude/one/SKILL.md'][0]
        print('public-only positive + unknown preservation + owned-only retirement: PASS')
        print('fixture only; no catalogue registration, harness settings or installed acceptance')


if __name__ == '__main__':
    main()
