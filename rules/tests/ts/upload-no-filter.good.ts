import multer from "multer";
export const upload = multer({
  dest: "uploads/",
  limits: { fileSize: 2 * 1024 * 1024 },
  fileFilter: (_req, file, cb) => cb(null, ["image/png", "image/jpeg"].includes(file.mimetype)),
});
