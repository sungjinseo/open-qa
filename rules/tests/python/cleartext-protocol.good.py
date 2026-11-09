import ftplib

def upload(host, user, pw):
    ftp = ftplib.FTP_TLS(host)
    ftp.login(user, pw)
