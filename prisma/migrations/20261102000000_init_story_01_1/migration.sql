-- CreateTable: projects
CREATE TABLE "projects" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "name" TEXT NOT NULL,
    "slug" TEXT NOT NULL,
    "created_at" BIGINT NOT NULL,
    "updated_at" BIGINT NOT NULL
);

-- CreateIndex
CREATE UNIQUE INDEX "projects_slug_key" ON "projects"("slug");

-- CreateTable: project_specs
CREATE TABLE "project_specs" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "project_id" TEXT NOT NULL,
    "spec_key" TEXT NOT NULL,
    "status" TEXT NOT NULL DEFAULT 'CONFIRMED',
    "updated_at" BIGINT NOT NULL,
    CONSTRAINT "project_specs_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "projects" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateIndex
CREATE INDEX "project_specs_project_id_idx" ON "project_specs"("project_id");
CREATE UNIQUE INDEX "project_specs_project_id_spec_key_key" ON "project_specs"("project_id", "spec_key");

-- CreateTable: spec_revisions
CREATE TABLE "spec_revisions" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "spec_id" TEXT NOT NULL,
    "revision_number" INTEGER NOT NULL,
    "content_hash" TEXT NOT NULL,
    "created_at" BIGINT NOT NULL,
    CONSTRAINT "spec_revisions_spec_id_fkey" FOREIGN KEY ("spec_id") REFERENCES "project_specs" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateIndex
CREATE INDEX "spec_revisions_spec_id_idx" ON "spec_revisions"("spec_id");
CREATE UNIQUE INDEX "spec_revisions_spec_id_revision_number_key" ON "spec_revisions"("spec_id", "revision_number");
