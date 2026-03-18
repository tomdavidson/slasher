const { execSync } = require('child_process')
const lib = require('./lib')

const collectTargets = deployments =>
  deployments.map(d => JSON.parse(d.payload || '{}')).flatMap(p => p.projects ?? []).map((
    { id, rm, artifact_name },
  ) => ({ id, rm, artifact_name }))

const checkAffectedMismatch = (targets, core) => {
  let raw
  try {
    raw = execSync('moon query projects --affected').toString()
  } catch (error) {
    core.warning(`Could not check affected projects: ${error.message}`)
    return false
  }

  const { projects } = JSON.parse(raw)
  const affectedIds = new Set(projects.map(p => p.id))
  const stale = targets.filter(t => !affectedIds.has(t.id))

  if (!stale.length) return true

  core.warning(`Teardown targets not in --affected: ${stale.map(t => t.id).join(', ')}`)
  return false
}

const runTeardowns = async (exec, core, targets) => {
  const results = []
  for (const target of targets) {
    const moonTarget = lib.targetName(target.id, target.rm)
    core.info(`Running teardown: ${moonTarget}`)
    try {
      await exec.exec('moon', ['run', moonTarget])
      results.push({ ...target, status: 'done' })
    } catch (error) {
      core.error(`Failed ${moonTarget}: ${error.message}`)
      results.push({ ...target, status: 'failed' })
    }
  }
  return results
}

module.exports = async ({ github, context, core, exec, inputs }) => {
  const { owner, repo } = context.repo
  const { pr_number, prefix } = inputs
  const environment = lib.environmentName(prefix, pr_number)

  const deployments = await lib.listDeployments(github, owner, repo, environment)

  if (!deployments.length) {
    core.info(`No deployments found for ${environment}`)
    core.setOutput('environment', environment)
    core.setOutput('project-count', '0')
    core.setOutput('summary', 'No RAA projects to tear down.')
    core.setOutput('manifest', '{}')
    core.setOutput('clean', 'true')
    return
  }

  const targets = collectTargets(deployments)
  const affectedClean = checkAffectedMismatch(targets, core)
  const results = await runTeardowns(exec, core, targets)

  await lib.deactivateDeployments(github, owner, repo, environment, core)
  await lib.deleteEnvironment(github, owner, repo, environment)
  core.info(`Cleaned up environment ${environment}`)

  const manifest = { pr_number, projects: targets }
  const summary = lib.formatRmSummary(environment, results)
  const allPassed = results.every(r => r.status === 'done')

  core.setOutput('environment', environment)
  core.setOutput('project-count', String(targets.length))
  core.setOutput('summary', summary)
  core.setOutput('manifest', JSON.stringify(manifest))
  core.setOutput('clean', String(affectedClean && allPassed))

  if (!allPassed) core.setFailed('One or more teardown tasks failed')
}
